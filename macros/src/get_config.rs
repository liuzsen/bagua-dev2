use quote::{quote, quote_spanned, ToTokens};
use syn::{parse::Parse, punctuated::Punctuated, spanned::Spanned, Field, Ident, Token};

pub struct GetConfig {
    ident: Ident,
    fields: FieldsNamed,
}
type FieldsNamed = Punctuated<Field, Token![,]>;

impl Parse for GetConfig {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = syn::DeriveInput::parse(input)?;
        match derive_input.data {
            syn::Data::Struct(data_struct) => match data_struct.fields {
                syn::Fields::Named(fields_named) => Ok(Self {
                    ident: derive_input.ident,
                    fields: fields_named.named,
                }),
                _ => {
                    return Err(syn::Error::new_spanned(
                        &derive_input.ident,
                        "expected named fields",
                    ));
                }
            },
            _ => {
                return Err(syn::Error::new_spanned(
                    &derive_input.ident,
                    "`GetConfig` expected struct",
                ));
            }
        }
    }
}

impl GetConfig {
    pub fn expand(&self) -> syn::Result<proc_macro2::TokenStream> {
        Ok(quote! {#self})
    }
}

impl ToTokens for GetConfig {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let this_ty = &self.ident;
        for field in &self.fields {
            let ty = &field.ty;
            let name = field.ident.as_ref().unwrap();
            let stream = quote_spanned! { field.span() =>
                impl ::bagua::configs::GetConfig<#ty> for #this_ty {
                    fn get_config(&self) -> &#ty {
                        &self.#name
                    }
                }
            };
            tokens.extend(stream);
        }
    }
}
