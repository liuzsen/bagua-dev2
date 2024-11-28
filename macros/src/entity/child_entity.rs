use convert_case::{Case, Casing};
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned, ToTokens};
use syn::{
    parse::{self, Parse},
    parse_quote,
    punctuated::Punctuated,
    token::Colon,
    Attribute, Data, Field, Ident, Token,
};

pub struct Entity {
    name: syn::Ident,
    attrs: Vec<syn::Attribute>,
    model_attrs: Vec<syn::Meta>,
    entity_attrs: Vec<syn::Meta>,
    updater_attrs: Vec<syn::Meta>,

    subsets: Vec<Subset>,

    all_fields: Vec<EntityField>,
}

#[derive(Debug, Clone)]
struct EntityField {
    inner: syn::Field,
    role: FieldRole,
    can_be_update: bool,
    update_with: Option<syn::Expr>,

    model_attrs: Vec<Attribute>,
    updater_attrs: Vec<Attribute>,
    entity_attrs: Vec<Attribute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldRole {
    NormalField,
    Foreign,
    Parent,
}

struct Subset {
    name: syn::Ident,
    fields: Vec<SubsetField>,
    unused_fields: Vec<Field>,
}

#[derive(Clone)]
struct SubsetField {
    ident: syn::Ident,
    ty: syn::Type,
    attrs: Vec<syn::Attribute>,
    is_manual_ty: bool,
}

type Fields = Punctuated<Field, Token![,]>;

impl Entity {
    pub fn expand(&self) -> syn::Result<TokenStream2> {
        let model = self.expand_model()?;
        let updater = self.expand_updater()?;
        let entity = self.expand_entity()?;
        let subsets = self.expand_subsets()?;

        let output = quote::quote_spanned! { self.name.span() =>
            #model
            #updater
            #entity
            #subsets
        };

        Ok(output)
    }

    fn expand_subsets(&self) -> syn::Result<TokenStream2> {
        let entity_name = &self.name;
        let mut subsets = vec![];

        for subset in &self.subsets {
            let name = &subset.name;
            let subset_fields: Fields =
                subset.fields.iter().map(|field| field.to_field()).collect();

            let to_entity_fields = subset.fields.iter().map(|field| {
                    let ident = &field.ident;
                    if field.is_manual_ty {
                        let origin_field = self.all_fields.iter().find(|f| f.inner.ident.as_ref().unwrap() == &field.ident).unwrap();
                        let origin_field_ty = &origin_field.inner.ty;
                        quote! {
                            #ident: ::bagua::entity::field::Unchanged::unchanged(<#origin_field_ty>::from(self.#ident)),
                        }
                    } else {
                        quote! {
                            #ident: ::bagua::entity::field::Unchanged::unchanged(self.#ident),
                        }
                    }
                }).collect::<Vec<_>>();
            let unused_fields = subset
                .unused_fields
                .iter()
                .map(|field| {
                    let ident = &field.ident;
                    quote! {
                        #ident: ::bagua::entity::field::Unloaded::unloaded(),
                    }
                })
                .collect::<Vec<_>>();

            let subset = quote! {
                pub struct #name {
                    #subset_fields,
                }

                impl ::bagua::entity::subset::Subset for #name {
                    type Entity = #entity_name;

                    fn to_entity(self) -> Self::Entity {
                        #entity_name {
                            #(#to_entity_fields)*
                            #(#unused_fields)*
                        }
                    }
                }
            };
            subsets.push(subset);
        }

        let subset_mod_name = syn::Ident::new(
            &format!("{}_subsets", self.name).to_case(Case::Snake),
            proc_macro2::Span::call_site(),
        );
        let output = quote_spanned! { self.name.span() =>
            #[allow(unused)]
            pub use #subset_mod_name::*;
            pub mod #subset_mod_name {
                #![allow(unused_imports)]
                use super::*;

                #(#subsets)*
            }
        };
        Ok(output)
    }

    fn expand_model(&self) -> syn::Result<TokenStream2> {
        let fields: Punctuated<ModelField, Token![,]> = self
            .all_fields
            .iter()
            .map(|field| field.to_model_field())
            .collect();
        let model_ident = format!("{}Model", self.name);
        let model_ident = syn::Ident::new(&model_ident, self.name.span());
        let attrs = &self.attrs;
        let model_attrs = &self.model_attrs;

        let map_fields = fields.iter().map(|field| {
            let ident = field.inner.ident.as_ref().unwrap();
            match field.kind {
                ModelFieldKind::Normal => {
                    quote! {
                        #ident: bagua::entity::field::Reset::reset(self.#ident),
                    }
                }
                ModelFieldKind::Parent => {
                    quote! {
                        #ident: bagua::entity::field::Reset::reset(self.#ident.build_entity()),
                    }
                }
            }
        });
        let entity_ident = &self.name;
        let span = entity_ident.span();
        let output = quote_spanned! { span =>
            #(#attrs)*
            #(#[#model_attrs])*
            #[allow(unused)]
            pub struct #model_ident {
                #fields
            }

            const _: () = {
                use bagua::entity::model::Model;
                impl Model for #model_ident {
                    type Entity = #entity_ident;

                    fn build_entity(self) -> Self::Entity {
                        #entity_ident {
                            #(#map_fields)*
                        }
                    }
                }
            };

            impl #model_ident {
                #[allow(unused)]
                pub fn build_entity(self) -> #entity_ident {
                    bagua::entity::model::Model::build_entity(self)
                }
            }
        };
        Ok(output)
    }

    fn impl_guarded_struct(&self) -> TokenStream2 {
        let entity_name = &self.name;
        let updater_ident = self.updater_type();
        let entity_trait = quote! {
            const _: () = {
                impl bagua::entity::GuardedStruct for #entity_name {
                    type Updater = #updater_ident;

                    fn update_fields(&mut self, updater: Self::Updater) {
                        self.update_fields(updater);
                    }
                }
            };
        };
        entity_trait
    }

    fn expand_entity(&self) -> syn::Result<TokenStream2> {
        let fields: Punctuated<Field, Token![,]> = self
            .all_fields
            .iter()
            .map(|field| field.to_guarded_field())
            .collect();
        let entity_ident = &self.name;
        let attrs = &self.attrs;
        let entity_attrs = &self.entity_attrs;

        let entity_repr = self.entity_repr();
        let read_only_struct = self.read_only_struct(fields.clone(), &entity_repr)?;
        let read_only_ident = &read_only_struct.name;

        let impl_deref = self.impl_deref(read_only_ident);
        let impl_entity_trait = self.impl_entity_trait();
        let impl_guarded = self.impl_guarded_struct();

        let output = quote_spanned! { self.name.span() =>
            #(#attrs)*
            #(#[#entity_attrs])*
            #entity_repr
            pub struct #entity_ident {
                #fields
            }

            #impl_entity_trait

            #impl_guarded

            #impl_deref

            #read_only_struct

            impl #entity_ident {
                pub fn read_only(&self) -> &#read_only_ident {
                    ::core::ops::Deref::deref(self)
                }
            }
        };
        Ok(output)
    }

    fn expand_updater(&self) -> syn::Result<TokenStream2> {
        let entity_ident = &self.name;
        let ident = self.updater_type();
        let fields: Punctuated<UpdaterField, Token![,]> = self
            .all_fields
            .iter()
            .flat_map(|field| field.to_updater_fields())
            .collect();
        let update_statement = fields.iter().map(|field| field.update_statement());
        let attrs = &self.attrs;
        let updater_attrs = &self.updater_attrs;

        let output = quote_spanned! { self.name.span() =>
            #(#attrs)*
            #(#[#updater_attrs])*
            #[derive(Default)]
            pub struct #ident {
                #fields
            }

            impl bagua::entity::updater::Updater for #ident {
                type GuardedStruct = #entity_ident;
            }

            impl #entity_ident {
                pub fn update_fields(&mut self, updater: #ident) {
                    #[allow(unused_imports)]
                    use bagua::entity::GuardedStruct as _;

                    #(#update_statement)*
                }
            }
        };

        Ok(output)
    }

    fn updater_type(&self) -> Ident {
        let ident = Ident::new(&format!("{}Updater", self.name), self.name.span());
        ident
    }

    fn entity_repr(&self) -> syn::Attribute {
        let repr = self
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("repr"))
            .cloned();
        repr.unwrap_or_else(|| syn::parse_quote!(#[repr(C)]))
    }

    fn read_only_struct(
        &self,
        mut guarded_fileds: Fields,
        repr: &syn::Attribute,
    ) -> syn::Result<ReadOnlyEntity> {
        let name = syn::Ident::new(&format!("{}ReadOnly", self.name), self.name.span());
        guarded_fileds.iter_mut().for_each(|field| {
            field.vis = pub_vis();
        });
        let mut attrs = self.attrs.clone();
        attrs.push(repr.clone());

        Ok(ReadOnlyEntity {
            name,
            attrs,
            fields: guarded_fileds,
        })
    }

    fn impl_entity_trait(&self) -> TokenStream2 {
        let entity_ident = &self.name;
        let parent = self
            .all_fields
            .iter()
            .find(|f| f.role == FieldRole::Parent)
            .unwrap();
        let parent_ty = &parent.inner.ty;
        let entity_trait = quote! {
            const _: () = {
                use bagua::entity::Entity;
                impl Entity for #entity_ident {
                    type Id<'a> = <#parent_ty as Entity>::Id<'a>;

                    type SysId = <#parent_ty as Entity>::SysId;
                }
            };
        };
        entity_trait
    }

    fn impl_deref(&self, read_only_ident: &Ident) -> TokenStream2 {
        let name = &self.name;
        quote! {
            impl core::ops::Deref for #name {
                type Target = #read_only_ident;

                fn deref(&self) -> &Self::Target {
                    // Two repr(C) structs with the same fields are guaranteed to
                    // have the same layout.
                    unsafe { &*(self as *const Self as *const Self::Target) }
                }
            }
        }
    }
}

fn pub_vis() -> syn::Visibility {
    syn::Visibility::Public(Token![pub](proc_macro2::Span::call_site()))
}

struct UpdaterField {
    field: Field,
    role: UpdaterFieldRole,
    update_with: Option<syn::Expr>,
}

enum UpdaterFieldRole {
    Normal,
    Foreign,
    Parent,
    ForeignAdd(Ident),
    ForeignRemove(Ident),
}

pub struct ReadOnlyEntity {
    name: syn::Ident,
    attrs: Vec<Attribute>,
    fields: Punctuated<Field, Token![,]>,
}

impl ToTokens for ReadOnlyEntity {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let ReadOnlyEntity {
            name,
            attrs,
            fields,
        } = self;
        let output = quote_spanned! { self.name.span() =>
            #(#attrs)*
            pub struct #name {
                #fields
            }
        };
        output.to_tokens(tokens);
    }
}

struct ModelField {
    inner: Field,
    kind: ModelFieldKind,
}

enum ModelFieldKind {
    Normal,
    Parent,
}

impl ToTokens for ModelField {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        self.inner.to_tokens(tokens);
    }
}

impl EntityField {
    fn field_ident(&self) -> &Ident {
        self.inner.ident.as_ref().unwrap()
    }

    fn to_model_field(&self) -> ModelField {
        let mut field = self.inner.clone();
        field.attrs.extend(self.model_attrs.clone());
        let span = self.inner.ident.as_ref().unwrap().span();
        field.vis = syn::Visibility::Public(Token![pub](span));
        let kind;
        match self.role {
            FieldRole::Parent => match &mut field.ty {
                syn::Type::Path(p) => {
                    let last = &mut p.path.segments.last_mut().unwrap().ident;
                    let new_ident = Ident::new(&format!("{}Model", last), last.span());
                    *last = new_ident;
                    kind = ModelFieldKind::Parent;
                }
                _ => panic!("parent filed must be a Path"),
            },
            _ => {
                kind = ModelFieldKind::Normal;
            }
        }

        ModelField { inner: field, kind }
    }

    fn to_guarded_field(&self) -> Field {
        let mut field = self.inner.clone();
        field.attrs.extend(self.entity_attrs.clone());
        field.vis = syn::Visibility::Inherited;

        let ty = &field.ty;
        match self.role {
            FieldRole::NormalField => {
                let guarded_ty = parse_quote!(bagua::entity::field::Field::<#ty>);
                field.ty = guarded_ty;
            }
            FieldRole::Foreign => {
                let guarded_ty = parse_quote!(bagua::entity::foreign::ForeignEntities::<#ty>);
                field.ty = guarded_ty;
            }
            FieldRole::Parent => {
                let guarded_ty = parse_quote!(bagua::entity::parent::ParentEntity::<#ty>);
                field.ty = guarded_ty;
            }
        }

        field
    }

    fn to_updater_fields(&self) -> Vec<UpdaterField> {
        if !self.can_be_update {
            return vec![];
        }

        let inner = self.inner.clone();
        let origin_ident = inner.ident.as_ref().unwrap();
        let field = {
            let mut field = self.inner.clone();
            let ty = &field.ty;
            field.ty = parse_quote!(Option<#ty>);
            field.vis = pub_vis();
            field.attrs.extend(self.updater_attrs.clone());
            field
        };

        match self.role {
            FieldRole::NormalField => {
                let field = UpdaterField {
                    field,
                    role: UpdaterFieldRole::Normal,
                    update_with: self.update_with.clone(),
                };
                vec![field]
            }
            FieldRole::Foreign => {
                let mut fields = vec![];

                fields.push(UpdaterField {
                    field: field.clone(),
                    role: UpdaterFieldRole::Foreign,
                    update_with: self.update_with.clone(),
                });

                fields.push(UpdaterField {
                    field: {
                        let mut field = field.clone();
                        let field_name = format!("add_{}", origin_ident);
                        field.ident = Some(Ident::new(&field_name, origin_ident.span()));
                        field
                    },
                    role: UpdaterFieldRole::ForeignAdd(origin_ident.clone()),
                    update_with: self.update_with.clone(),
                });
                fields.push(UpdaterField {
                    field: {
                        let mut field = field.clone();
                        let field_name = format!("remove_{}", origin_ident);
                        field.ident = Some(Ident::new(&field_name, origin_ident.span()));
                        field
                    },
                    role: UpdaterFieldRole::ForeignRemove(origin_ident.clone()),
                    update_with: self.update_with.clone(),
                });
                fields
            }
            FieldRole::Parent => {
                let mut field = field;
                let mut ty = inner.ty.clone();
                match &mut ty {
                    syn::Type::Path(p) => {
                        let last = &mut p.path.segments.last_mut().unwrap().ident;
                        let new_ident = Ident::new(&format!("{}Updater", last), last.span());
                        *last = new_ident;
                    }
                    _ => unreachable!(),
                }
                let new_ty = parse_quote!(#ty);
                field.ty = new_ty;
                let field = UpdaterField {
                    field,
                    role: UpdaterFieldRole::Parent,
                    update_with: self.update_with.clone(),
                };
                vec![field]
            }
        }
    }
}

impl UpdaterField {
    fn update_statement(&self) -> TokenStream2 {
        match &self.role {
            UpdaterFieldRole::Normal | UpdaterFieldRole::Foreign => {
                if let Some(update_with) = &self.update_with {
                    quote! {#update_with}
                } else {
                    let field_name = &self.field.ident;
                    quote! {
                        self.#field_name.update_value(updater.#field_name);
                    }
                }
            }
            UpdaterFieldRole::ForeignAdd(origin_ident) => {
                let updater_ident = self.field.ident.as_ref().unwrap();
                quote! {
                    if let Some(values) = updater.#updater_ident {
                        for t in values {
                            self.#origin_ident.add(t);
                        }
                    }
                }
            }
            UpdaterFieldRole::ForeignRemove(origin_ident) => {
                let updater_ident = self.field.ident.as_ref().unwrap();
                quote! {
                    if let Some(values) = updater.#updater_ident {
                        for t in values {
                            self.#origin_ident.remove(t);
                        }
                    }
                }
            }
            UpdaterFieldRole::Parent => {
                if let Some(update_with) = &self.update_with {
                    quote! {#update_with}
                } else {
                    let field_name = &self.field.ident;
                    quote! {
                        self.#field_name.update_fields(updater.#field_name);
                    }
                }
            }
        }
    }
}

impl ToTokens for UpdaterField {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        self.field.to_tokens(tokens)
    }
}

impl SubsetField {
    fn to_field(&self) -> Field {
        Field {
            attrs: self.attrs.clone(),
            vis: pub_vis(),
            mutability: syn::FieldMutability::None,
            ident: Some(self.ident.clone()),
            colon_token: Some(Colon::default()),
            ty: self.ty.clone(),
        }
    }
}

impl Parse for Entity {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input = input.parse::<syn::DeriveInput>()?;
        let data = match &input.data {
            Data::Struct(data) => data,
            _ => return Err(syn::Error::new_spanned(input, "expected struct")),
        };

        let fields: Vec<_> = match &data.fields {
            syn::Fields::Named(fields) => fields
                .named
                .iter()
                .cloned()
                .map(|field| {
                    let field = parse_entity_filed(field)?;

                    Ok(field)
                })
                .collect::<syn::Result<Vec<_>>>()?,
            _ => return Err(syn::Error::new_spanned(input, "expected named fields")),
        };

        if !fields.iter().any(|f| f.role == FieldRole::Parent) {
            return Err(syn::Error::new_spanned(input, "expected one parent field"));
        }

        let mut subsets = vec![];
        let mut original_attrs = vec![];
        let mut model_attrs = vec![];
        let mut entity_attrs = vec![];
        let mut updater_attrs = vec![];
        let attrs = input.attrs.clone();
        for attr in attrs {
            if attr.path().is_ident("subset") {
                parse_subset_attr(&attr, &fields, &mut subsets)?;
            } else if attr.path().is_ident("model_attr") {
                let derive = attr.parse_args::<syn::Meta>()?;
                model_attrs.push(derive);
            } else if attr.path().is_ident("entity_attr") {
                let derive = attr.parse_args::<syn::Meta>()?;
                entity_attrs.push(derive);
            } else if attr.path().is_ident("updater_attr") {
                let derive = attr.parse_args::<syn::Meta>()?;
                updater_attrs.push(derive);
            } else {
                original_attrs.push(attr);
            }
        }

        Ok(Self {
            name: input.ident,
            all_fields: fields,
            attrs: original_attrs,
            subsets,
            model_attrs,
            entity_attrs,
            updater_attrs,
        })
    }
}

enum FieldAttr {
    Mark(syn::Ident),
    UpdateWith(syn::Expr),
}

impl Parse for FieldAttr {
    fn parse(input: parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "update_with" {
            let _eq = input.parse::<Token![=]>()?;
            let expr = input.parse::<syn::Expr>()?;
            Ok(Self::UpdateWith(expr))
        } else {
            Ok(Self::Mark(ident))
        }
    }
}

fn extract_nested_attr(attr: &syn::Attribute) -> syn::Result<Attribute> {
    let meta = attr.meta.clone();
    match meta {
        syn::Meta::List(p) => {
            let nested = p.tokens;
            let nested = syn::parse2::<syn::Meta>(nested)?;
            Ok(Attribute {
                pound_token: attr.pound_token,
                style: attr.style,
                bracket_token: attr.bracket_token,
                meta: nested,
            })
        }
        syn::Meta::Path(p) => Err(syn::Error::new_spanned(p, "expected nested attribute")),
        syn::Meta::NameValue(_) => Err(syn::Error::new_spanned(meta, "expected nested attribute")),
    }
}

fn parse_entity_filed(mut field: Field) -> syn::Result<EntityField> {
    let mut origin_attrs = vec![];
    let mut filed_attrs = vec![];
    let mut entity_attrs = vec![];
    let mut model_attrs = vec![];
    let mut updater_attrs = vec![];
    for attr in &field.attrs {
        if attr.path().is_ident("entity") {
            let a = attr.parse_args_with(<Punctuated<FieldAttr, Token![,]>>::parse_terminated)?;
            filed_attrs.extend(a);
        } else if attr.path().is_ident("model_attr") {
            model_attrs.push(extract_nested_attr(attr)?);
        } else if attr.path().is_ident("entity_attr") {
            entity_attrs.push(extract_nested_attr(attr)?);
        } else if attr.path().is_ident("updater_attr") {
            updater_attrs.push(extract_nested_attr(attr)?);
        } else {
            origin_attrs.push(attr.clone());
        }
    }
    let mut can_be_update = true;
    let mut field_role = FieldRole::NormalField;
    let mut update_with = None;
    for attr in filed_attrs {
        match attr {
            FieldAttr::Mark(mark) => match &*mark.to_string() {
                "foreign" => {
                    field_role = FieldRole::Foreign;
                }
                "no_update" => {
                    can_be_update = false;
                }
                "parent" => {
                    field_role = FieldRole::Parent;
                }
                _ => {
                    return Err(syn::Error::new_spanned(mark, "unknown field mark"));
                }
            },
            FieldAttr::UpdateWith(u) => {
                update_with = Some(u);
            }
        }
    }
    field.attrs = origin_attrs;
    let field = EntityField {
        inner: field.clone(),
        role: field_role,
        can_be_update,
        update_with,
        model_attrs,
        updater_attrs,
        entity_attrs,
    };
    Ok(field)
}

fn parse_subset_attr(
    attr: &syn::Attribute,
    fields: &Vec<EntityField>,
    subsets: &mut Vec<Subset>,
) -> syn::Result<()> {
    let subset = attr.parse_args::<SubsetRaw>()?;

    let mut used_fields = vec![];
    let mut used_indices = vec![];
    for subset_field in subset.fields {
        let position = fields
            .iter()
            .position(|f| f.field_ident() == &subset_field.name);
        let Some(position) = position else {
            let msg = format!("field `{}` not found in entity", subset_field.name);
            return Err(syn::Error::new_spanned(subset_field.name, msg));
        };
        used_indices.push(position);
        let field = &fields[position];

        let is_manual_ty = subset_field.ty.is_some();
        let ty = match subset_field.ty {
            Some(t) => t,
            None => field.inner.ty.clone(),
        };

        used_fields.push(SubsetField {
            ident: field.field_ident().clone(),
            ty,
            attrs: field.inner.attrs.clone(),
            is_manual_ty,
        });
    }

    let unused_fields = fields
        .iter()
        .enumerate()
        .filter(|(index, _)| !used_indices.contains(index))
        .map(|(_, f)| f.inner.clone())
        .collect::<Vec<_>>();

    let subset = Subset {
        name: subset.name,
        fields: used_fields,
        unused_fields,
    };
    subsets.push(subset);
    Ok(())
}

struct SubsetRaw {
    name: syn::Ident,
    fields: Vec<SubsetFieldRaw>,
}

#[derive(Clone)]
struct SubsetFieldRaw {
    name: syn::Ident,
    ty: Option<syn::Type>,
}

impl Parse for SubsetFieldRaw {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let ty = if input.peek(Token![:]) {
            let _colon = input.parse::<Token![:]>()?;
            let ty = input.parse::<syn::Type>()?;
            Some(ty)
        } else {
            None
        };

        Ok(Self { name, ty })
    }
}

impl Parse for SubsetRaw {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let fields;
        syn::braced!(fields in input);
        let fields = fields.parse_terminated(SubsetFieldRaw::parse, Token![,])?;

        Ok(Self {
            name,
            fields: fields.into_iter().collect(),
        })
    }
}
