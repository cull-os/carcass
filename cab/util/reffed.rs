//! Macro helpers for paired owned/reference enums.

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
      $crate::private::paste! {
         $(#[$attribute])*
         $vis enum $name {
            $(
               $(#[$variant_attribute])*
               $variant($type),
            )*
         }

         impl $name {
            #[must_use]
            $vis fn as_ref(&self) -> [<$name Ref>]<'_> {
               match *self {
                  $(Self::$variant(ref variant) => [<$name Ref>]::$variant(variant),)*
               }
            }
         }

         $(#[$attribute])*
         #[derive(Copy)]
         $vis enum [<$name Ref>]<'a> {
            $(
               $(#[$variant_attribute])*
               $variant(&'a $type),
            )*
         }

         impl [<$name Ref>]<'_> {
            #[must_use]
            $vis fn to_owned(self) -> $name {
               match self {
                  $(Self::$variant(variant) => $name::$variant(variant.clone()),)*
               }
            }
         }
      }
   };
}
