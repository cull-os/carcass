use proc_macro as pm;
use quote::quote;

#[proc_macro_derive(Dupe)]
pub fn dupe_derive(input: pm::TokenStream) -> pm::TokenStream {
   let input = syn::parse_macro_input!(input as syn::DeriveInput);
   let name = &input.ident;

   let members: Vec<_> = match input.data {
      syn::Data::Struct(struct_) => struct_.fields.into_iter().map(|field| field.ty).collect(),

      syn::Data::Enum(enum_) => {
         enum_
            .variants
            .into_iter()
            .flat_map(|variant| variant.fields.into_iter().map(|field| field.ty))
            .collect()
      },

      syn::Data::Union(union_) => {
         union_
            .fields
            .named
            .into_iter()
            .map(|field| field.ty)
            .collect()
      },
   };

   pm::TokenStream::from(quote! {
      impl ::dup::Dupe for #name
      where
         #(#members: ::dup::Dupe,)*
      {}
   })
}
