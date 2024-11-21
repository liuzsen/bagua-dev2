use convert_case::{Case, Casing};
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{parse::Parse, punctuated::Punctuated, Token};

pub struct InitMethod {
    inner: syn::ItemFn,
    arg_name: syn::Pat,
    arg_ty: syn::TypeReference,
}

pub struct ExtFields {
    fields: Punctuated<ExtField, Token![,]>,
}

pub struct ExtField {
    name: syn::Ident,
    value: syn::Expr,
}

impl ToTokens for ExtField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let value = &self.value;
        tokens.extend(quote! {
           .#name(#value)
        });
    }
}

impl Parse for ExtField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let value = input.parse()?;

        Ok(Self { name, value })
    }
}

impl Parse for ExtFields {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let fields = Punctuated::<ExtField, Token![,]>::parse_terminated(input)?;
        Ok(Self { fields })
    }
}

impl Parse for InitMethod {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let inner = input.parse::<syn::ItemFn>()?;
        if inner.sig.asyncness.is_none() {
            return Err(syn::Error::new_spanned(
                &inner.sig,
                "init method must be async",
            ));
        }

        let Some(arg) = inner.sig.inputs.iter().next() else {
            return Err(syn::Error::new_spanned(
                &inner.sig,
                "init method must have exactly one argument",
            ));
        };
        let arg = arg.clone();
        let arg_ty;
        let pat;
        match arg {
            syn::FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    &receiver,
                    "init method must not have a receiver",
                ));
            }
            syn::FnArg::Typed(pat_type) => {
                pat = *pat_type.pat;
                match *pat_type.ty {
                    syn::Type::Reference(ty) => {
                        arg_ty = ty;
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &pat_type.ty,
                            "argument must be a reference",
                        ));
                    }
                }
            }
        }

        Ok(Self {
            inner,
            arg_name: pat,
            arg_ty,
        })
    }
}

impl InitMethod {
    pub fn expand(&self, ext_fields: ExtFields) -> syn::Result<proc_macro2::TokenStream> {
        let ident = &self.inner.sig.ident;
        let body = &self.inner.block;
        let mut arg_ty = self.arg_ty.clone();
        arg_ty.lifetime = Some(syn::Lifetime::new("'a", proc_macro2::Span::call_site()));
        let arg_name = &self.arg_name;
        let pkg_name = match std::env::var("CARGO_PKG_NAME") {
            Ok(n) => n,
            Err(_) => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "env CARGO_PKG_NAME is not exists",
                ));
            }
        };
        let pkg_name = pkg_name.to_case(Case::ScreamingSnake);
        let slice_ident = format!("{pkg_name}_INIT_FUNCTIONS");
        let slice_ident = syn::Ident::new(&slice_ident, Span::call_site());

        let fields = &ext_fields.fields.iter().collect::<Vec<_>>();
        let s = quote::quote! {
            fn #ident <'a> (#arg_name: #arg_ty)
                -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = ::anyhow::Result<()>> + 'a>>
            {
                const _: () = {
                    use crate::init::InitFunction;
                    use crate::init::InitFunctionBuilder;

                    #[::linkme::distributed_slice(crate::init::#slice_ident)]
                    static INIT: InitFunction = {
                        let mut builder = InitFunctionBuilder::new(#ident);
                        builder = builder;
                        #(builder = builder #fields ;)*
                        builder.build()
                    };
                };
                Box::pin(async move #body)
            }

        };

        Ok(s)
    }
}
