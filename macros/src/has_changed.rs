use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{parse::Parse, DeriveInput, Field, Generics, Ident};

pub struct Struct {
    ident: Ident,
    generics: Generics,
    fields: Vec<StructField>,
}

struct StructField {
    field: Field,
    kind: FieldKind,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FieldKind {
    Option,
    Id,
    Value,
}

impl Parse for Struct {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input = DeriveInput::parse(input)?;
        let ds = match input.data {
            syn::Data::Struct(ds) => ds,
            syn::Data::Enum(_) => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Enums are not supported to derive HasChanged",
                ))
            }
            syn::Data::Union(_) => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Unions are not supported to derive HasChanged",
                ))
            }
        };

        let fields = match ds.fields {
            syn::Fields::Named(fields_named) => fields_named,
            syn::Fields::Unnamed(_fields_unnamed) => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "Struct with unnamed fields are not supported to derive HasChanged",
                ));
            }
            syn::Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "unit structs are not supported to derive HasChanged",
                ));
            }
        };

        let fields = fields
            .named
            .into_iter()
            .map(StructField::from_syn)
            .collect();
        let this = Struct {
            ident: input.ident,
            generics: input.generics,
            fields,
        };

        Ok(this)
    }
}

impl StructField {
    fn from_syn(field: Field) -> StructField {
        let field_ident = field.ident.as_ref().unwrap();
        let field_kind = if field_ident == "id" {
            FieldKind::Id
        } else {
            match &field.ty {
                syn::Type::Path(path) => {
                    let segments = &path.path.segments;
                    if segments.len() == 1 && segments[0].ident == "Option" {
                        FieldKind::Option
                    } else {
                        FieldKind::Value
                    }
                }
                _ => FieldKind::Value,
            }
        };

        StructField {
            field,
            kind: field_kind,
        }
    }
}

impl Struct {
    pub fn expand(&self) -> syn::Result<TokenStream> {
        let (impl_generics, ty_generics, where_clause) = &self.generics.split_for_impl();
        let option_fields = self.fields.iter().filter_map(|f| match f.kind {
            FieldKind::Option => Some(&f.field.ident),
            FieldKind::Id => None,
            FieldKind::Value => None,
        });

        let check_expr = if self.has_value_field() {
            quote! {
                true
            }
        } else {
            quote! {
                #(self.#option_fields.is_some()) ||*
            }
        };

        let struct_ident = &self.ident;
        let stream = quote_spanned! { struct_ident.span() =>
            impl #impl_generics #struct_ident #ty_generics #where_clause {
                pub fn has_changed(&self) -> bool {
                    #check_expr
                }
            }
        };

        Ok(stream)
    }

    fn has_value_field(&self) -> bool {
        self.fields.iter().any(|f| f.kind == FieldKind::Value)
    }
}
