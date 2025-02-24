use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
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

        if fields.is_empty() {
            return Err(syn::Error::new_spanned(
                input,
                "expected at least one named field",
            ));
        }

        let mut subsets = vec![];
        let mut original_attrs = vec![];
        let mut model_attrs = vec![];
        let mut entity_attrs = vec![];
        let mut updater_attrs = vec![];
        let attrs = input.attrs.clone();
        for attr in attrs {
            let Some(attr_ident) = attr.path().get_ident() else {
                original_attrs.push(attr);
                continue;
            };
            let attr_ident = attr_ident.to_string();
            match &*attr_ident {
                "subset" => {
                    parse_subset_attr(&attr, &fields, &mut subsets, &fields[0])?;
                }
                "model_attr" => {
                    let derive = attr.parse_args::<syn::Meta>()?;
                    model_attrs.push(derive);
                }
                "entity_attr" => {
                    let derive = attr.parse_args::<syn::Meta>()?;
                    entity_attrs.push(derive);
                }
                "updater_attr" => {
                    let derive = attr.parse_args::<syn::Meta>()?;
                    updater_attrs.push(derive);
                }
                _ => original_attrs.push(attr),
            }
        }

        let this = Self {
            name: input.ident,
            all_fields: fields,
            attrs: original_attrs,
            subsets,
            model_attrs,
            entity_attrs,
            updater_attrs,
        };

        Ok(this)
    }
}

pub struct ReadOnlyEntity {
    name: syn::Ident,
    attrs: Vec<Attribute>,
    fields: Punctuated<Field, Token![,]>,
}

struct Subset {
    name: syn::Ident,
    fields: Vec<SubsetField>,
    unused_fields: Vec<Field>,
}

#[derive(Debug, Clone)]
struct EntityField {
    origin: syn::Field,
    kind: FieldKind,
    no_update: bool,

    model_attrs: Vec<Attribute>,
    updater_attrs: Vec<Attribute>,
    entity_attrs: Vec<Attribute>,
}

#[derive(Clone, Debug)]
struct SubsetField {
    ident: syn::Ident,
    ty: syn::Type,
    attrs: Vec<syn::Attribute>,
    is_manual_ty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldKind {
    Scalar,
    Foreign,
    Group,
}
impl FieldKind {
    fn is_group(&self) -> bool {
        matches!(self, FieldKind::Group)
    }
}

impl Entity {
    pub fn expand(self) -> syn::Result<TokenStream> {
        let mut tokens = TokenStream::new();
        tokens.extend(self.expand_model()?);
        tokens.extend(self.expand_updater()?);
        tokens.extend(self.expand_entity()?);
        tokens.extend(self.expand_subsets()?);

        Ok(tokens)
    }

    fn expand_model(&self) -> syn::Result<TokenStream> {
        let entity_name = &self.name;
        let model_name = self.model_name();
        let model_fields = self.all_fields.iter().map(|f| f.to_model_field());

        let field_inits = self.all_fields.iter().map(|f| f.model_to_entity());
        let field_names = self.all_fields.iter().map(|f| f.ident());

        let serde_attrs = serde_derive_attrs();
        let model_attrs = &self.model_attrs;
        let attrs = self.attrs.iter().chain(serde_attrs.iter());

        let stream = quote::quote_spanned! { self.name.span() =>
            #(#attrs)*
            #(#[#model_attrs])*
            pub struct #model_name {
                #(#model_fields),*
            }

            const _: () = {
                impl #model_name {
                    fn build_entity(self) -> #entity_name {
                        #entity_name {
                            #(#field_names: #field_inits)*
                        }
                    }
                }
            };
        };

        Ok(stream)
    }

    fn expand_updater(&self) -> syn::Result<TokenStream> {
        let entity_name = &self.name;
        let updater_name = self.updater_name();
        let updater_fields = self
            .all_fields
            .iter()
            .flat_map(|f| f.to_updater_field())
            .collect::<Vec<_>>();
        let update_statements = updater_fields
            .clone()
            .into_iter()
            .map(|f| f.update_statement());

        let serde_attrs = serde_derive_attrs();
        let attrs = self.attrs.iter().chain(serde_attrs.iter());
        let updater_attrs = &self.updater_attrs;

        let stream = quote_spanned! { self.name.span() =>
            #(#attrs)*
            #(#[#updater_attrs])*
            #[derive(Default)]
            pub struct #updater_name {
                #(#updater_fields,)*
            }

            impl bagua::entity::updater::Updater for #updater_name {
                type FieldGroup = #entity_name;
            }

            impl #entity_name {
                pub fn update_fields(&mut self, updater: #updater_name) {
                    #(#update_statements)*
                }
            }
        };

        Ok(stream)
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
        mut guarded_fields: Fields,
        repr: &syn::Attribute,
    ) -> syn::Result<ReadOnlyEntity> {
        let name = syn::Ident::new(&format!("{}ReadOnly", self.name), self.name.span());
        guarded_fields.iter_mut().for_each(|field| {
            field.vis = pub_vis();
        });
        let mut attrs = self.attrs.clone();
        attrs.push(repr.clone());

        Ok(ReadOnlyEntity {
            name,
            attrs,
            fields: guarded_fields,
        })
    }

    fn impl_deref(&self, read_only_ident: &Ident) -> TokenStream {
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

    fn impl_field_group(&self) -> TokenStream {
        let entity_name = &self.name;
        let updater_ident = self.updater_name();
        let subset_full_ident = self.subset_full_ident();
        let entity_trait = quote! {
            const _: () = {
                impl bagua::entity::FieldGroup for #entity_name {
                    type Updater = #updater_ident;
                    type SubsetFull = #subset_full_ident;

                    fn update_fields(&mut self, updater: Self::Updater) {
                        self.update_fields(updater);
                    }
                }
            };
        };
        entity_trait
    }

    fn expand_entity(&self) -> syn::Result<TokenStream> {
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
        let impl_field_group = self.impl_field_group();
        let impl_unloaded = self.impl_unloaded();

        let stream = quote_spanned! { self.name.span() =>
            #(#attrs)*
            #(#[#entity_attrs])*
            #entity_repr
            pub struct #entity_ident {
                #fields
            }

            #impl_field_group

            #impl_deref

            #read_only_struct

            impl #entity_ident {
                pub fn read_only(&self) -> &#read_only_ident {
                    ::core::ops::Deref::deref(self)
                }
            }

            #impl_unloaded
        };
        Ok(stream)
    }

    fn subset_full_ident(&self) -> Ident {
        crate::entity::entity::subset_full_ident(&self.name)
    }

    fn default_subsets(&self) -> Vec<Subset> {
        let full = Subset {
            name: self.subset_full_ident(),
            fields: self
                .all_fields
                .iter()
                .map(|f| f.to_subset_field())
                .collect(),
            unused_fields: vec![],
        };

        vec![full]
    }

    fn impl_unloaded(&self) -> TokenStream {
        let entity_name = &self.name;
        let fields = self
            .all_fields
            .iter()
            .map(|f| {
                let field = f.to_guarded_field();
                let ident = &field.ident;
                quote! {
                    #ident: ::bagua::entity::field::Unloaded::unloaded(),
                }
            })
            .collect::<Vec<_>>();

        quote! {
            const _: () = {
                impl bagua::entity::field::Unloaded for #entity_name {
                    fn unloaded() -> Self {
                        #entity_name {
                            #(#fields)*
                        }
                    }
                }
            };
        }
    }

    fn expand_subsets(&self) -> syn::Result<TokenStream> {
        let entity_name = &self.name;

        let mut subsets = vec![];

        for subset in self.subsets.iter().chain(self.default_subsets().iter()) {
            let name = &subset.name;
            let subset_fields: Fields =
                subset.fields.iter().map(|field| field.to_field()).collect();

            let to_entity_fields = subset.fields.iter().map(|field| {
                    let ident = &field.ident;
                    if field.is_manual_ty {
                        let origin_field = self.all_fields.iter().find(|f| f.origin.ident.as_ref().unwrap() == &field.ident).unwrap();
                        let origin_field_ty = &origin_field.origin.ty;
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

                impl #name {
                    fn to_entity(self) -> #entity_name {
                        #entity_name {
                            #(#to_entity_fields)*
                            #(#unused_fields)*
                        }
                    }
                }

                impl From<#name> for #entity_name {
                    fn from(subset: #name) -> Self {
                        subset.to_entity()
                    }
                }

                impl bagua::entity::field::Unchanged<#entity_name> for #name {
                    fn unchanged(self) -> #entity_name {
                        self.to_entity()
                    }
                }
            };
            subsets.push(subset);
        }

        let subset_mod_name = syn::Ident::new(
            &format!("{}_subsets", self.name).to_case(Case::Snake),
            proc_macro2::Span::call_site(),
        );
        let stream = quote_spanned! { self.name.span() =>
            #[allow(unused)]
            pub use #subset_mod_name::*;
            pub mod #subset_mod_name {
                #![allow(unused_imports)]
                use super::*;

                #(#subsets)*
            }
        };
        Ok(stream)
    }
}

impl Entity {
    fn model_name(&self) -> syn::Ident {
        model_struct_name(&self.name)
    }

    fn updater_name(&self) -> syn::Ident {
        syn::Ident::new(&format!("{}Updater", self.name), self.name.span())
    }
}

fn model_struct_name(entity_name: &Ident) -> Ident {
    Ident::new(&format!("{}Model", entity_name), entity_name.span())
}

impl EntityField {
    fn to_guarded_field(&self) -> Field {
        let mut field = self.origin.clone();
        field.attrs.extend(self.entity_attrs.clone());
        field.vis = syn::Visibility::Inherited;

        let ty = &field.ty;
        match self.kind {
            FieldKind::Scalar => {
                let guarded_ty = parse_quote!(bagua::entity::field::Field::<#ty>);
                field.ty = guarded_ty;
            }
            FieldKind::Foreign => {
                let guarded_ty = parse_quote!(bagua::entity::foreign::ForeignEntities::<#ty>);
                field.ty = guarded_ty;
            }
            FieldKind::Group => {
                let guarded_ty = parse_quote!(bagua::entity::flatten::FlattenStruct::<#ty>);
                field.ty = guarded_ty;
            }
        }

        field
    }

    fn to_subset_field(&self) -> SubsetField {
        let ty = match self.kind {
            FieldKind::Group => {
                let ty = self.ty().clone();
                parse_quote!(<#ty as bagua::entity::FieldGroup>::SubsetFull)
            }
            _ => self.ty().clone(),
        };
        SubsetField {
            ident: self.ident().clone(),
            ty,
            attrs: self.origin.attrs.clone(),
            is_manual_ty: false,
        }
    }

    /// Generate a model field for this entity field.
    ///
    /// # Panics
    ///
    /// - If the field is an id field.
    fn to_model_field(&self) -> Field {
        let mut field = self.origin.clone();
        field.attrs.extend(self.model_attrs.clone());
        let span = self.origin.ident.as_ref().unwrap().span();
        field.vis = syn::Visibility::Public(Token![pub](span));

        match self.kind {
            FieldKind::Scalar => {}
            FieldKind::Foreign => {}
            FieldKind::Group => match &mut field.ty {
                syn::Type::Path(p) => {
                    let last = &mut p.path.segments.last_mut().unwrap().ident;
                    let new_ident = model_struct_name(last);
                    *last = new_ident;
                }
                _ => panic!("flatten field must be a path"),
            },
        }

        field
    }

    fn model_to_entity(&self) -> TokenStream {
        let ident = self.ident();
        match self.kind {
            FieldKind::Group => {
                quote! {
                     bagua::entity::field::Reset::reset(self.#ident.build_entity()),
                }
            }
            _ => {
                quote! {
                    bagua::entity::field::Reset::reset(self.#ident),
                }
            }
        }
    }

    fn ident(&self) -> &Ident {
        self.origin.ident.as_ref().unwrap()
    }

    fn ty(&self) -> &syn::Type {
        &self.origin.ty
    }

    fn to_updater_field(&self) -> Vec<UpdaterField> {
        if self.no_update {
            return vec![];
        }

        let mut inner = self.origin.clone();
        inner.attrs.extend(self.updater_attrs.clone());
        let origin_ident = inner.ident.as_ref().unwrap();

        let field = {
            let mut field = self.origin.clone();
            let ty = &field.ty;
            field.ty = parse_quote!(Option<#ty>);
            field.vis = pub_vis();
            field.attrs.extend(self.updater_attrs.clone());

            if !self.kind.is_group() {
                let serde_attrs = [
                    parse_quote!(#[serde(default)]),
                    parse_quote!(#[serde(deserialize_with = "bagua::entity::updater::de_double_option")]),
                ];
                field.attrs.extend(serde_attrs);
            }

            field
        };

        match self.kind {
            FieldKind::Scalar => {
                let field = UpdaterField {
                    field,
                    role: UpdaterFieldKind::Scalar,
                };
                vec![field]
            }
            FieldKind::Foreign => {
                let mut fields = vec![];

                fields.push(UpdaterField {
                    field: field.clone(),
                    role: UpdaterFieldKind::Foreign,
                });

                fields.push(UpdaterField {
                    field: {
                        let mut field = field.clone();
                        let field_name = format!("add_{}", origin_ident);
                        field.ident = Some(Ident::new(&field_name, origin_ident.span()));
                        field
                    },
                    role: UpdaterFieldKind::ForeignAdd(origin_ident.clone()),
                });
                fields.push(UpdaterField {
                    field: {
                        let mut field = field.clone();
                        let field_name = format!("remove_{}", origin_ident);
                        let origin_ty = &self.origin.ty;
                        field.ty = parse_quote!{
                           Option<std::collections::HashSet<<<#origin_ty as bagua::entity::foreign::ForeignContainer>::Item as bagua::entity::foreign::ForeignEntity>::Id>>
                        };
                        field.ident = Some(Ident::new(&field_name, origin_ident.span()));
                        field
                    },
                    role: UpdaterFieldKind::ForeignRemove(origin_ident.clone()),
                });

                fields
            }
            FieldKind::Group => {
                let mut field = field;
                let mut ty = inner.ty.clone();
                match &mut ty {
                    syn::Type::Path(p) => {
                        let last = &mut p.path.segments.last_mut().unwrap().ident;
                        let new_ident = Ident::new(&format!("{}Updater", last), last.span());
                        *last = new_ident;
                    }
                    _ => panic!("flatten field must be a path"),
                }
                let new_ty = parse_quote!(#ty);
                field.ty = new_ty;
                let field = UpdaterField {
                    field,
                    role: UpdaterFieldKind::Group,
                };
                vec![field]
            }
        }
    }
}

#[derive(Clone)]
struct UpdaterField {
    field: Field,
    role: UpdaterFieldKind,
}

#[derive(Clone)]
enum UpdaterFieldKind {
    Scalar,
    Foreign,
    ForeignAdd(Ident),
    ForeignRemove(Ident),
    Group,
}

impl UpdaterField {
    fn update_statement(&self) -> TokenStream {
        let field_name = &self.field.ident;
        match &self.role {
            UpdaterFieldKind::Scalar | UpdaterFieldKind::Foreign => {
                quote! {
                    self.#field_name.update_value(updater.#field_name);
                }
            }
            UpdaterFieldKind::ForeignAdd(origin_ident) => {
                let updater_ident = self.field.ident.as_ref().unwrap();
                quote! {
                    if let Some(values) = updater.#updater_ident {
                        for t in values {
                            self.#origin_ident.add(t);
                        }
                    }
                }
            }
            UpdaterFieldKind::ForeignRemove(origin_ident) => {
                let updater_ident = self.field.ident.as_ref().unwrap();
                quote! {
                    if let Some(values) = updater.#updater_ident {
                        for t in values {
                            self.#origin_ident.remove(t);
                        }
                    }
                }
            }
            UpdaterFieldKind::Group => {
                quote! {
                    self.#field_name.update_fields(updater.#field_name);
                }
            }
        }
    }
}

impl ToTokens for UpdaterField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.field.to_tokens(tokens)
    }
}

fn pub_vis() -> syn::Visibility {
    syn::Visibility::Public(Token![pub](proc_macro2::Span::call_site()))
}

type Fields = Punctuated<Field, Token![,]>;

impl ToTokens for ReadOnlyEntity {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ReadOnlyEntity {
            name,
            attrs,
            fields,
        } = self;
        let stream = quote_spanned! { self.name.span() =>
            #(#attrs)*
            pub struct #name {
                #fields
            }
        };
        stream.to_tokens(tokens);
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

fn parse_entity_filed(mut field: Field) -> syn::Result<EntityField> {
    let mut origin_attrs = vec![];
    let mut filed_attrs = vec![];
    let mut entity_attrs = vec![];
    let mut model_attrs = vec![];
    let mut updater_attrs = vec![];
    for attr in &field.attrs {
        let ident = attr.path().get_ident().map(|i| i.to_string());
        let Some(ident_str) = ident else {
            origin_attrs.push(attr.clone());
            continue;
        };

        match &*ident_str {
            "entity" => {
                let a =
                    attr.parse_args_with(<Punctuated<FieldAttr, Token![,]>>::parse_terminated)?;
                filed_attrs.extend(a);
            }
            "model_attr" => {
                model_attrs.push(extract_nested_attr(attr)?);
            }
            "entity_attr" => {
                entity_attrs.push(extract_nested_attr(attr)?);
            }
            "updater_attr" => {
                updater_attrs.push(extract_nested_attr(attr)?);
            }
            _ => {
                origin_attrs.push(attr.clone());
            }
        }
    }
    let mut no_update = false;
    let mut field_role = FieldKind::Scalar;
    for attr in filed_attrs {
        match attr {
            FieldAttr::Mark(mark) => match &*mark.to_string() {
                "foreign" => {
                    field_role = FieldKind::Foreign;
                }
                "group" => {
                    field_role = FieldKind::Group;
                }
                "no_update" => {
                    no_update = true;
                }
                _ => {
                    return Err(syn::Error::new_spanned(mark, "unknown field mark"));
                }
            },
        }
    }
    field.attrs = origin_attrs;
    let field = EntityField {
        origin: field.clone(),
        kind: field_role,
        no_update,
        model_attrs,
        updater_attrs,
        entity_attrs,
    };
    Ok(field)
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

enum FieldAttr {
    Mark(syn::Ident),
}

impl Parse for FieldAttr {
    fn parse(input: parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        Ok(Self::Mark(ident))
    }
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

fn parse_subset_attr(
    attr: &syn::Attribute,
    fields: &Vec<EntityField>,
    subsets: &mut Vec<Subset>,
    id_field: &EntityField,
) -> syn::Result<()> {
    let mut subset = attr.parse_args::<SubsetRaw>()?;

    let had_id_field = subset.fields.iter().any(|f| &f.name == id_field.ident());
    if !had_id_field {
        subset.fields.insert(
            0,
            SubsetFieldRaw {
                name: id_field.ident().clone(),
                ty: None,
            },
        );
    }
    let mut used_fields = vec![];
    let mut used_indices = vec![];
    for subset_field in subset.fields {
        let position = fields.iter().position(|f| f.ident() == &subset_field.name);
        let Some(position) = position else {
            let msg = format!("field `{}` not found in entity", subset_field.name);
            return Err(syn::Error::new_spanned(subset_field.name, msg));
        };
        used_indices.push(position);
        let field = &fields[position];

        let is_manual_ty = subset_field.ty.is_some();
        let ty = match subset_field.ty {
            Some(t) => t,
            None => field.ty().clone(),
        };

        used_fields.push(SubsetField {
            ident: field.ident().clone(),
            ty,
            attrs: field.origin.attrs.clone(),
            is_manual_ty,
        });
    }

    let unused_fields = fields
        .iter()
        .enumerate()
        .filter(|(index, _)| !used_indices.contains(index))
        .map(|(_, f)| f.origin.clone())
        .collect::<Vec<_>>();

    let subset = Subset {
        name: subset.name,
        fields: used_fields,
        unused_fields,
    };
    subsets.push(subset);
    Ok(())
}

fn serde_derive_attrs() -> Vec<syn::Attribute> {
    vec![
        syn::parse_quote! {
            #[derive(serde::Serialize, serde::Deserialize)]
        },
        syn::parse_quote! {
            #[serde(rename_all = "camelCase")]
        },
    ]
}
