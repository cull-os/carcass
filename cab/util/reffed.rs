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
