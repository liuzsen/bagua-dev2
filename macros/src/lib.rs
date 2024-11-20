use proc_macro::TokenStream;

mod provider;

#[proc_macro_derive(Provider, attributes(provider))]
pub fn derive_provider(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    provider::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
