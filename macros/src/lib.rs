use proc_macro::TokenStream;

mod entity;
mod get_config;
mod has_changed;
mod provider;

#[proc_macro_derive(Provider, attributes(provider))]
pub fn derive_provider(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    provider::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn Entity(_args: TokenStream, item: TokenStream) -> TokenStream {
    let entity = syn::parse_macro_input!(item as entity::entity::Entity);
    let output = entity
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error);

    let stream: TokenStream = output.into();

    stream
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn GuardedStruct(_args: TokenStream, item: TokenStream) -> TokenStream {
    let entity = syn::parse_macro_input!(item as entity::field_guard::Entity);
    let output = entity
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error);

    let stream: TokenStream = output.into();

    stream
}

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn ChildEntity(_args: TokenStream, item: TokenStream) -> TokenStream {
    let entity = syn::parse_macro_input!(item as entity::child_entity::Entity);
    let output = entity
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error);

    let stream: TokenStream = output.into();

    stream
}

#[proc_macro_derive(HasChanged)]
pub fn derive_has_changed(input: TokenStream) -> TokenStream {
    let entity = syn::parse_macro_input!(input as has_changed::Struct);
    entity
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(GetConfig)]
pub fn derive_get_config(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as get_config::GetConfig);
    input
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
