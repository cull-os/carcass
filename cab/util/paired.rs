//! Macro helpers for paired owned/reference enums.

/// Declares an enum and an associated transformed enum generated as
/// `<Name><Suffix>`.
///
/// The macro generates:
/// - the primary enum exactly as declared,
/// - a companion enum where each payload type is transformed with
///   `transform_prefix` and `transform_suffix`,
/// - a forward transform method on the primary enum,
/// - a backward transform method on the companion enum.
#[macro_export]
macro_rules! paired {
   (
      suffix: $suffix:ident$(<$lifetime:lifetime>)?,
      transform_prefix: $transform_prefix:tt,
      transform_suffix: $transform_suffix:tt,
      into_paired: $into_paired_method:ident($into_paired_variant:ident $(, $into_paired_argument_name:ident: $into_paired_argument_type:ty)* $(,)?) $into_paired_expression:block,
      from_paired: $from_paired_method:ident($from_paired_variant:ident $(, $from_paired_argument_name:ident: $from_paired_argument_type:ty)* $(,)?) $from_paired_expression:block,
      attributes: ($($companion_attribute:tt)*),
      $(#[$attribute:meta])*
      $visibility:vis enum $name:ident {
         $(
            $(#[$variant_attribute:meta])*
            $variant:ident($type:ty)
         ),* $(,)?
      }
   ) => {
      $(#[$attribute])*
      $visibility enum $name {
         $(
            $(#[$variant_attribute])*
            $variant($type),
         )*
      }

      $crate::private::paste! {
         impl $name {
            $visibility fn $into_paired_method$(<$lifetime>)?(
               &$( $lifetime )? self
               $(, $into_paired_argument_name: $into_paired_argument_type)*
            ) -> [<$name $suffix>]$(<$lifetime>)? {
               match *self {
                  $(Self::$variant(ref variant) => {
                     let $into_paired_variant = variant;
                     [<$name $suffix>]::$variant($into_paired_expression)
                  },)*
               }
            }
         }

         $($companion_attribute)*
         $visibility enum [<$name $suffix>]$(<$lifetime>)? {
            $(
               $(#[$variant_attribute])*
               $variant($crate::paired!(@splice_transform $transform_prefix ; $type ; $transform_suffix)),
            )*
         }

         impl$(<$lifetime>)? [<$name $suffix>]$(<$lifetime>)? {
            $visibility fn $from_paired_method(
               self
               $(, $from_paired_argument_name: $from_paired_argument_type)*
            ) -> $name {
               match self {
                  $(Self::$variant(variant) => {
                     let $from_paired_variant = variant;
                     $name::$variant($from_paired_expression)
                  },)*
               }
            }
         }
      }
   };

   (@splice_transform ($($transform_prefix:tt)*) ; $type:ty ; ($($transform_suffix:tt)*)) => {
      $($transform_prefix)* $type $($transform_suffix)*
   };
}

/// Declares an enum and an associated borrowed enum generated as `<Name>Ref`.
///
/// The macro generates:
/// - the owned enum exactly as declared,
/// - a borrowed companion enum where each payload is `&'a T`,
/// - `as_ref(&self) -> <Name>Ref<'_>` on the owned enum,
/// - `to_owned(self) -> <Name>` on the borrowed enum.
///
/// `to_owned` clones each payload, so every variant payload type must implement
/// [`Clone`] for that method to compile.
///
/// # Example
///
/// ```rs
/// # use cab_util::reffed;
/// reffed! {
///    #[derive(Clone)]
///    pub enum Token {
///       Word(String),
///       Number(u32),
///    }
/// }
///
/// let token = Token::Word(String::from("doodoo"));
/// let token_ref = token.as_ref();
/// assert!(matches!(token_ref, TokenRef::Word(_)));
/// assert!(matches!(token_ref.to_owned(), Token::Word(_)));
/// ```
#[macro_export]
macro_rules! reffed {
   (
      $(#[$attribute:meta])*
      $vis:vis enum $name:ident {
         $(
            $(#[$variant_attribute:meta])*
            $variant:ident($type:ty)
         ),* $(,)?
      }
   ) => {
      $crate::paired! {
         suffix: Ref<'a>,
         transform_prefix: (&'a ),
         transform_suffix: (),
         into_paired: as_ref(variant) { variant },
         from_paired: to_owned(variant) { variant.clone() },
         attributes: (
            $(#[$attribute])*
            #[derive(Copy)]
         ),
         $(#[$attribute])*
         $vis enum $name {
            $(
               $(#[$variant_attribute])*
               $variant($type)
            ),*
         }
      }
   };
}
