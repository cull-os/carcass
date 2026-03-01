#![expect(dead_code)]

use std::borrow::Cow;

use derive_more::{
   Deref,
   From,
};
use ranged::Spanned;

const EXPECT_ARENA: &str = "expression must be in arena";

#[derive(Deref, Debug, Clone, Copy)]
pub struct Resolved<'arena, T> {
   arena: &'arena slotmap::SlotMap<ExpressionId, Expression>,
   #[deref]
   value: T,
}

impl<'arena, T> Resolved<'arena, T> {
   pub(crate) fn new(arena: &'arena slotmap::SlotMap<ExpressionId, Expression>, value: T) -> Self {
      Self { arena, value }
   }
}

macro_rules! lode {
   ($name:ident { $($field:tt)* }) => {
      lode! {
         @parse
         $name
         []
         []
         $($field)*
      }
   };

   (
      @parse
      $name:ident
      [ $($field_declaration:tt)* ]
      [ $($field_getter:tt)* ]
   ) => {
      #[derive(Debug, Clone, PartialEq, Eq)]
      pub struct $name {
         $($field_declaration)*
      }

      impl<'arena> Resolved<'arena, &'arena Spanned<$name>> {
         $($field_getter)*
      }
   };

   (
      @parse
      $name:ident
      [ $($field_declaration:tt)* ]
      [ $($field_getter:tt)* ]
      $field:ident
      $(, $($rest:tt)*)?
   ) => {
      lode! {
         @parse
         $name
         [
            $($field_declaration)*
            pub(crate) $field: ExpressionId,
         ]
         [
            $($field_getter)*
            get! { &'arena $field }
         ]
         $($($rest)*)?
      }
   };

   (
      @parse
      $name:ident
      [ $($field_declaration:tt)* ]
      [ $($field_getter:tt)* ]
      Option<$field:ident>
      $(, $($rest:tt)*)?
   ) => {
      lode! {
         @parse
         $name
         [
            $($field_declaration)*
            pub(crate) $field: Option<ExpressionId>,
         ]
         [
            $($field_getter)*
            get! { Option < &'arena $field > }
         ]
         $($($rest)*)?
      }
   };

   (
      @parse
      $name:ident
      [ $($field_declaration:tt)* ]
      [ $($field_getter:tt)* ]
      [$field:ident]
      $(, $($rest:tt)*)?
   ) => {
      lode! {
         @parse
         $name
         [
            $($field_declaration)*
            pub(crate) $field: Vec<ExpressionId>,
         ]
         [
            $($field_getter)*
            get! { [ &'arena $field ] }
         ]
         $($($rest)*)?
      }
   };
}

macro_rules! get {
   (&$lifetime:lifetime $field:ident) => {
      pub fn $field(&self) -> Resolved<$lifetime, &$lifetime Expression> {
         Resolved::new(self.arena, self.arena.get(self.$field).expect(EXPECT_ARENA))
      }
   };

   (Option < &$lifetime:lifetime $field:ident >) => {
      pub fn $field(&self) -> Option<Resolved<$lifetime, &$lifetime Expression>> {
         self
            .$field
            .map(|expression| Resolved::new(self.arena, self.arena.get(expression).expect(EXPECT_ARENA)))
      }
   };


   ([ &$lifetime:lifetime $field:ident ]) => {
      pub fn $field(&self) -> impl Iterator<Item = Resolved<$lifetime, &$lifetime Expression>> {
         self
            .$field
            .iter()
            .map(|&item| Resolved::new(self.arena, self.arena.get(item).expect(EXPECT_ARENA)))
      }
   };
}

// EXPRESSION

slotmap::new_key_type! {
   pub(crate) struct ExpressionId;
}

pub type Expression = Spanned<ExpressionRaw>;
#[derive(Debug, Clone, PartialEq, From)]
pub enum ExpressionRaw {
   Parenthesis(Parenthesis),
   List(List),
   Attributes(Attributes),

   Same(Same),
   Sequence(Sequence),
   Call(Call),
   Construct(Construct),
   Select(Select),
   Equal(Equal),
   And(And),
   Or(Or),
   All(All),
   Any(Any),
   Lambda(Lambda),

   Path(Path),
   Bind(Bind),
   Identifier(Identifier),
   SString(SString),

   Char(char),
   Integer(num::BigInt),
   Float(f64),

   If(If),
}

// PARENTHESIS

lode! { Parenthesis { expression } }

// LIST

lode! { List { [items] } }

// ATTRIBUTES

lode! { Attributes { Option<expression> } }

// OPERATIONS

lode! { Same { left, right } }
lode! { Sequence { left, right } }

lode! { Call { function, argument } }

lode! { Construct { head, tail } }

lode! { Select { scope, expression } }

lode! { Equal { left, right } }

lode! { And { left, right } }
lode! { Or { left, right } }

lode! { All { left, right } }
lode! { Any { left, right } }

lode! { Lambda { argument, expression } }

// SEGMENTED

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment<'content, I> {
   Content(Spanned<Cow<'content, str>>),
   Interpolation(I),
}

impl<I> Segment<'_, I> {
   #[must_use]
   fn is_content(&self) -> bool {
      matches!(self, Self::Content(..))
   }

   #[must_use]
   fn is_interpolation(&self) -> bool {
      matches!(self, Self::Interpolation(..))
   }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segments(pub(crate) Vec<Segment<'static, ExpressionId>>);

impl<'content, 'arena> IntoIterator for Resolved<'arena, &'content Segments> {
   type Item = Segment<'content, &'arena Expression>;

   type IntoIter = impl Iterator<Item = Self::Item>;

   fn into_iter(self) -> Self::IntoIter {
      self.0.iter().map(move |segment| {
         match segment {
            &Segment::Content(ref content) => {
               Segment::Content(content.as_ref().map(|inner| Cow::Borrowed(&**inner)))
            },
            &Segment::Interpolation(expression) => {
               Segment::Interpolation(self.arena.get(expression).expect(EXPECT_ARENA))
            },
         }
      })
   }
}

impl Segments {
   pub fn plain(content: Spanned<impl Into<Cow<'static, str>>>) -> Self {
      Self(vec![Segment::Content(content.map(Into::into))])
   }

   #[must_use]
   pub fn is_trivial(&self) -> bool {
      let &[ref segment] = &*self.0 else {
         return false;
      };

      segment.is_content()
   }
}

macro_rules! segmented {
   ($name:ident) => {
      #[derive(Deref, Debug, Clone, PartialEq, Eq)]
      pub struct $name(pub Segments);

      impl<'arena> Resolved<'arena, $name> {
         pub fn segments(&self) -> Resolved<'arena, &'_ Segments> {
            Resolved::new(self.arena, &self.0)
         }
      }
   };
}

segmented! { Path }
segmented! { Bind }
segmented! { Identifier }
segmented! { SString }

lode! { If { condition, consequence, alternative } }
