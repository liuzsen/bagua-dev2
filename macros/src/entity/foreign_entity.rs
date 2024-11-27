use quote::quote;
use syn::{parse::Parse, DeriveInput, Field};

pub struct ForeignEntity {
    entity_ident: syn::Ident,
    id_ty: syn::Type,
    id_ident: syn::Ident,
}

impl Parse for ForeignEntity {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input = DeriveInput::parse(input)?;

        let fields = match input.data {
            syn::Data::Struct(data_struct) => match data_struct.fields {
                syn::Fields::Named(fields_named) => fields_named,
                _ => {
                    return Err(syn::Error::new_spanned(
                        &input.ident,
                        "expected named fields",
                    ));
                }
            },
            _ => {
                return Err(syn::Error::new_spanned(
                    &input.ident,
                    "`ForeignEntity` expected struct",
                ));
            }
        };

        let entity_ident = input.ident;
        let id_field = fields
            .named
            .iter()
            .find(is_attr_id)
            .cloned()
            .or_else(|| fields.named.iter().find(is_id_name).cloned());
        let id_field = match id_field {
            Some(f) => f,
            None => {
                return Err(syn::Error::new_spanned(
                        &entity_ident,
                        "The ForeignEntity must have one id field. You can use `#[foreign(id)]` attribute to define it or just use `id` as a field.",
                    ));
            }
        };

        Ok(Self {
            entity_ident,
            id_ty: id_field.ty,
            id_ident: id_field.ident.unwrap(),
        })
    }
}

fn is_attr_id(field: &&Field) -> bool {
    let attrs = &field.attrs;
    for attr in attrs {
        if attr.path().is_ident("foreign") {
            let Ok(a) = attr.parse_args::<syn::Ident>() else {
                return false;
            };
            if a == "id" {
                return true;
            }
        }
    }

    false
}

fn is_id_name(field: &&Field) -> bool {
    let ident = field.ident.as_ref().unwrap();
    ident == "id"
}

impl ForeignEntity {
    pub fn expand(self) -> syn::Result<proc_macro2::TokenStream> {
        let ForeignEntity {
            id_ty,
            id_ident,
            entity_ident,
        } = self;

        let stream = quote! {
            impl std::cmp::Eq for #entity_ident {}

            impl std::cmp::PartialEq for #entity_ident {
                fn eq(&self, other: &Self) -> bool {
                    self.#id_ident == other.#id_ident
                }
            }

            impl std::hash::Hash for #entity_ident {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    self.#id_ident.hash(state);
                }
            }

            impl ForeignEntity for #entity_ident {
                type Id = #id_ty;
            }

            impl Borrow<#id_ty> for #entity_ident {
                fn borrow(&self) -> &#id_ty {
                    &self.#id_ident
                }
            }


        };

        Ok(stream)
    }
}
