use proc_macro as pm;
use quote::quote;

#[proc_macro_derive(Dupe)]
pub fn dupe_derive(input: pm::TokenStream) -> pm::TokenStream {
   let input = syn::parse_macro_input!(input as syn::DeriveInput);
   let name = &input.ident;

   let expanded = quote! {
      impl ::dup::Dupe for #name {}
   };

   pm::TokenStream::from(expanded)
}
