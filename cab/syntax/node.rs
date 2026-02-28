//! Typed [`Node`] definitions.
//!
//! [`Node`]: crate::Node
use std::{
   collections::VecDeque,
   ops,
   ptr,
};

use cab_util::{
   force,
   lazy,
   read,
   ready,
   reffed,
};
use derive_more::Deref;
use dup::Dupe;
use paste::paste;
use ranged::{
   IntoSpan as _,
   Span,
};
pub use segment::{
   Segment,
   Segmented,
};
use ust::report::Report;

use crate::{
   Kind::{
      self,
      *,
   },
   red,
   token,
};

macro_rules! node {
   (
      #[from($kind:ident)]
      $(#[$attribute:meta])*
      struct $name:ident;
   ) => {
      $(#[$attribute])*
      #[derive(Deref, Debug, Clone, Dupe, PartialEq, Eq, Hash)]
      #[repr(transparent)]
      pub struct $name(red::Node);

      impl<'a> TryFrom<&'a red::Node> for &'a $name {
         type Error = ();

         fn try_from(node: &'a red::Node) -> Result<Self, ()> {
            if node.kind() != $kind {
               return Err(());
            }

            // SAFETY: node is &red::Node and we are casting it to &$name.
            // $name holds red::Node with #[repr(transparent)], so the layout
            // is the exact same for &red::Node and &$name.
            Ok(unsafe { &*ptr::from_ref(node).cast::<$name>() })
         }
      }

      impl TryFrom<red::Node> for $name {
         type Error = ();

         fn try_from(node: red::Node) -> Result<Self, ()> {
            if node.kind() != $kind {
               return Err(());
            }

            Ok(Self(node))
         }
      }

      impl $name {
         pub const KIND: Kind = $kind;
      }
   };

   (
      #[from($($variant:ident),* $(,)?)]
      $(#[$attribute:meta])*
      enum $name:ident;
   ) => {
      reffed! {
         $(#[$attribute])*
         #[derive(Debug, Clone, Dupe, PartialEq, Eq, Hash)]
         pub enum $name {
            $($variant($variant),)*
         }
      }

      impl ops::Deref for $name {
         type Target = red::Node;

         fn deref(&self) -> &Self::Target {
            match *self {
               $(Self::$variant(ref node) => &**node,)*
            }
         }
      }

      impl TryFrom<red::Node> for $name {
         type Error = ();

         fn try_from(node: red::Node) -> Result<Self, ()> {
            Ok(match node.kind() {
               $($variant::KIND => Self::$variant($variant::try_from(node)?),)*
               _ => return Err(()),
            })
         }
      }

      $(
         impl From<$variant> for $name {
            fn from(from: $variant) -> Self {
               Self::$variant(from)
            }
         }

         impl TryFrom<$name> for $variant {
            type Error = ();

            fn try_from(from: $name) -> Result<Self, ()> {
               if let $name::$variant(node) = from {
                  Ok(node)
               } else {
                  Err(())
               }
            }
         }
      )*

      paste! {
         impl ops::Deref for [<$name Ref>]<'_> {
            type Target = red::Node;

            fn deref(&self) -> &Self::Target {
               match *self {
                  $(Self::$variant(ref node) => &**node,)*
               }
            }
         }

         impl<'a> TryFrom<&'a red::Node> for [<$name Ref>]<'a> {
            type Error = ();

            fn try_from(node: &'a red::Node) -> Result<Self, ()> {
               Ok(match node.kind() {
                  $($variant::KIND => Self::$variant(<&$variant>::try_from(node)?),)*
                  _ => return Err(()),
               })
            }
         }

         $(
            impl<'a> From<&'a $variant> for [<$name Ref>]<'a> {
               fn from(from: &'a $variant) -> Self {
                  Self::$variant(from)
               }
            }

            impl<'a> TryFrom<[<$name Ref>]<'a>> for &'a $variant {
               type Error = ();

               fn try_from(from: [<$name Ref>]<'a>) -> Result<Self, ()> {
                  if let [<$name Ref>]::$variant(node) = from {
                     Ok(node)
                  } else {
                     Err(())
                  }
               }
            }
         )*
      }
   };
}

macro_rules! get_token {
   ($name:ident -> $($skip:literal @)? Option<$kind:ident>) => {
      #[must_use]
      pub fn $name(&self) -> Option<&red::Token> {
         self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            $(.skip($skip))?
            .find(|token| token.kind() == $kind)
      }
   };

   ($name:ident -> $($skip:literal @)? $kind:ident) => {
      #[must_use]
      pub fn $name(&self) -> &red::Token {
         self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            $(.skip($skip))?
            .find(|token| token.kind() == $kind)
            .expect("node must have a token child")
      }
   };

   ($name:ident -> $($skip:literal @)? Option<$type:ty>) => {
      #[must_use]
      pub fn $name(&self) -> $type {
         self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            $(.skip($skip))?
            .find_map(|token| <$type>::try_from(token).ok())
      }
   };

   ($name:ident -> $($skip:literal @)? $type:ty) => {
      #[must_use]
      pub fn $name(&self) -> $type {
         self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            $(.skip($skip))?
            .find_map(|token| <$type>::try_from(token).ok())
            .expect("node must have a token child")
      }
   };
}

macro_rules! get_node {
   ($name:ident -> $($skip:literal @)? Option<$type:ty>) => {
      #[must_use]
      pub fn $name(&self) -> Option<$type> {
         self.children()
            .filter_map(|node| <$type>::try_from(node).ok())
            $(.skip($skip))?
            .next()
      }
   };

   ($name:ident -> $($skip:literal @)? $type:ty) => {
      #[must_use]
      pub fn $name(&self) -> $type {
         self.children()
            .filter_map(|node| <$type>::try_from(node).ok())
            $(.skip($skip))?
            .next()
            .expect("node must have a node child of given type")
      }
   };
}

// EXPRESSION

node! {
   #[from(
      Error,

      Parenthesis,
      List,
      Attributes,

      PrefixOperation,
      InfixOperation,
      SuffixOperation,

      Path,

      Bind,
      Identifier,

      SString,

      Char,
      Integer,
      Float,

      If,
   )]
   /// An expression. Everything is an expression.
   enum Expression;
}

impl<'a> ExpressionRef<'a> {
   #[stacksafe::stacksafe]
   pub fn validate(self, to: &mut Vec<Report>) {
      match self {
         Self::Parenthesis(parenthesis) => parenthesis.validate(to),
         Self::List(list) => list.validate(to),
         Self::Attributes(attributes) => attributes.validate(to),
         Self::PrefixOperation(operation) => operation.validate(to),
         Self::InfixOperation(operation) => operation.validate(to),
         Self::SuffixOperation(operation) => operation.validate(to),
         Self::Path(path) => path.validate(to),
         Self::Bind(bind) => bind.validate(to),
         Self::Identifier(identifier) => identifier.validate(to),
         Self::SString(string) => string.validate(to),
         Self::Char(char) => char.validate(to),
         Self::Integer(integer) => integer.validate(to),
         Self::Float(float) => float.validate(to),
         Self::If(if_else) => if_else.validate(to),

         Self::Error(_) => {},
      }
   }

   /// Iterates over all subexpressions delimited with the same operator.
   pub fn same_items(self) -> impl Iterator<Item = ExpressionRef<'a>> {
      gen move {
         let mut expressions = VecDeque::from([self]);

         while let Some(expression) = expressions.pop_back() {
            match expression {
               ExpressionRef::InfixOperation(operation)
                  if let InfixOperator::Same = operation.operator() =>
               {
                  if let Some(left) = operation.left() {
                     expressions.push_front(left);
                  }
                  if let Some(right) = operation.right() {
                     expressions.push_front(right);
                  }
               },

               normal => yield normal,
            }
         }
      }
   }
}

// ERROR

node! {
   #[from(NODE_ERROR)]
   /// An error node. Also a valid expression.
   struct Error;
}

// PARENTHESIS

node! {
   #[from(NODE_PARENTHESIS)]
   /// A parenthesis. Contains a single expression.
   struct Parenthesis;
}

impl Parenthesis {
   get_token! { token_parenthesis_left -> TOKEN_PARENTHESIS_LEFT }

   get_node! { expression -> Option<ExpressionRef<'_>> }

   get_token! { token_parenthesis_right -> Option<TOKEN_PARENTHESIS_RIGHT> }

   pub fn validate(&self, to: &mut Vec<Report>) {
      match self.expression() {
         Some(expression) => {
            expression.validate(to);
         },

         None => {
            to.push(
               Report::error("parenthesis without inner expression").primary(
                  Span::empty(self.token_parenthesis_left().span().end),
                  "expected an expression here",
               ),
            );
         },
      }

      if self.token_parenthesis_right().is_none() {
         to.push(
            Report::error("unclosed parenthesis")
               .primary(Span::empty(self.span().end), "expected ')' here")
               .secondary(self.token_parenthesis_left().span(), "unclosed '(' here"),
         );
      }
   }
}

// LIST

node! {
   #[from(NODE_LIST)]
   /// A list. Contains a list of expressions delimited by the same operator.
   struct List;
}

impl List {
   get_token! { token_bracket_left -> TOKEN_BRACKET_LEFT }

   get_node! { expression -> Option<ExpressionRef<'_>> }

   get_token! { token_bracket_right -> Option<TOKEN_BRACKET_RIGHT> }

   /// Iterates over all the items of the list.
   pub fn items(&self) -> impl Iterator<Item = ExpressionRef<'_>> {
      self
         .expression()
         .into_iter()
         .flat_map(ExpressionRef::same_items)
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if let Some(ExpressionRef::InfixOperation(operation)) = self.expression()
         && operation.operator() == InfixOperator::Sequence
      {
         to.push(
            Report::error("inner expression of list cannot be sequence")
               .primary(operation.span(), "consider parenthesizing this"),
         );
      }

      for item in self.items() {
         item.validate(to);
      }

      if self.token_bracket_right().is_none() {
         to.push(
            Report::error("unclosed list")
               .primary(Span::empty(self.span().end), "expected ']' here")
               .secondary(self.token_bracket_left().span(), "unclosed '[' here"),
         );
      }
   }
}

// ATTRIBUTES

node! {
   #[from(NODE_ATTRIBUTES)]
   /// Attributes. May contain an expression that contains binds, which get appended to its scope.
   struct Attributes;
}

impl Attributes {
   get_token! { token_curlybrace_left -> TOKEN_CURLYBRACE_LEFT }

   get_node! { expression -> Option<ExpressionRef<'_>> }

   get_token! { token_curlybrace_right -> Option<TOKEN_CURLYBRACE_RIGHT> }

   pub fn validate(&self, to: &mut Vec<Report>) {
      // TODO: Warn for non-binding children.

      if self.token_curlybrace_right().is_none() {
         to.push(
            Report::error("unclosed attributes")
               .primary(Span::empty(self.span().end), "expected '}' here")
               .secondary(self.token_curlybrace_left().span(), "unclosed '{' here"),
         );
      }
   }
}

// PREFIX OPERATION

/// A prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
   Swwallation, // Get it?
   Negation,

   Not,
}

impl TryFrom<Kind> for PrefixOperator {
   type Error = ();

   fn try_from(from: Kind) -> Result<Self, ()> {
      Ok(match from {
         TOKEN_PLUS => Self::Swwallation,
         TOKEN_MINUS => Self::Negation,

         TOKEN_EXCLAMATION => Self::Not,

         _ => return Err(()),
      })
   }
}

impl PrefixOperator {
   /// Returns the binding power of this operator.
   #[must_use]
   pub fn binding_power(self) -> ((), u16) {
      match self {
         Self::Swwallation | Self::Negation => ((), 145),
         Self::Not => ((), 125),
      }
   }
}

node! {
   #[from(NODE_PREFIX_OPERATION)]
   /// A prefix operation.
   struct PrefixOperation;
}

impl PrefixOperation {
   get_node! { right -> 0 @ Option<ExpressionRef<'_>> }

   /// Returns the operator token of this operation.
   pub fn operator_token(&self) -> &red::Token {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find(|token| PrefixOperator::try_from(token.kind()).is_ok())
         .unwrap()
   }

   /// Returns the operator of this operation.
   pub fn operator(&self) -> PrefixOperator {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find_map(|token| PrefixOperator::try_from(token.kind()).ok())
         .unwrap()
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if let Some(right) = self.right() {
         right.validate(to);
      } else {
         // TODO: Temporary.
         to.push(
            Report::error("curried prefix functions aren't supported yet")
               .primary(self.span(), "unsupported")
               .tip("create a lambda instead"),
         );
      }
   }
}

// INFIX OPERATION

/// An infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfixOperator {
   Same,
   Sequence,

   ImplicitCall,
   Call,
   Pipe,

   Concat,
   Construct,

   Select,
   Update,

   LessOrEqual,
   Less,
   MoreOrEqual,
   More,

   Equal,
   NotEqual,

   And,
   Or,
   Implication,

   All,
   Any,

   Addition,
   Subtraction,
   Multiplication,
   Power,
   Division,

   Lambda,
}

impl TryFrom<Kind> for InfixOperator {
   type Error = ();

   fn try_from(from: Kind) -> Result<Self, ()> {
      Ok(match from {
         TOKEN_COMMA => Self::Same,
         TOKEN_SEMICOLON => Self::Sequence,

         kind if kind.is_argument() => Self::ImplicitCall,
         TOKEN_LESS_PIPE => Self::Call,
         TOKEN_PIPE_MORE => Self::Pipe,

         TOKEN_PLUS_PLUS => Self::Concat,
         TOKEN_COLON => Self::Construct,

         TOKEN_PERIOD => Self::Select,
         TOKEN_SLASH_SLASH => Self::Update,

         TOKEN_LESS_EQUAL => Self::LessOrEqual,
         TOKEN_LESS => Self::Less,
         TOKEN_MORE_EQUAL => Self::MoreOrEqual,
         TOKEN_MORE => Self::More,

         TOKEN_EQUAL => Self::Equal,
         TOKEN_EXCLAMATION_EQUAL => Self::NotEqual,

         TOKEN_AMPERSAND_AMPERSAND => Self::And,
         TOKEN_PIPE_PIPE => Self::Or,
         TOKEN_MINUS_MORE => Self::Implication,

         TOKEN_AMPERSAND => Self::All,
         TOKEN_PIPE => Self::Any,

         TOKEN_PLUS => Self::Addition,
         TOKEN_MINUS => Self::Subtraction,
         TOKEN_ASTERISK => Self::Multiplication,
         TOKEN_CARET => Self::Power,
         TOKEN_SLASH => Self::Division,

         TOKEN_EQUAL_MORE => Self::Lambda,

         _ => return Err(()),
      })
   }
}

impl InfixOperator {
   /// Returns the binding power of this operator.
   #[must_use]
   pub fn binding_power(self) -> (u16, u16) {
      match self {
            Self::Select => (185, 180),
            Self::ImplicitCall => (170, 175),

            Self::Concat => (160, 165),

            Self::Multiplication | Self::Division => (150, 155),
            Self::Power => (155, 150),

            // PrefixOperator::Swallation | PrefixOperator::Negation
            Self::Addition | Self::Subtraction => (130, 135),
            // PrefixOperator::Not
            Self::Update => (110, 115),

            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More /* | PrefixOperator::Try */ => {
                (100, 105)
            },

            Self::Construct => (95, 90),

            Self::And | Self::All => (85, 80),
            Self::Or | Self::Any => (75, 70),
            Self::Implication => (65, 60),

            Self::Pipe => (50, 55),
            Self::Call => (55, 50),

            Self::Lambda => (45, 40),

            Self::Equal | Self::NotEqual => (35, 30),

            Self::Same => (25, 20),
            Self::Sequence => (15, 10),
        }
   }

   /// Whether this operator actually owns a token. Not owning a token means
   /// that the operator doesn't actually "exist".
   #[must_use]
   pub fn is_token_owning(self) -> bool {
      self != Self::ImplicitCall
   }
}

node! {
   #[from(NODE_INFIX_OPERATION)]
   /// An infix operation.
   struct InfixOperation;
}

impl InfixOperation {
   #[must_use]
   pub fn left(&self) -> Option<ExpressionRef<'_>> {
      let operator_token = self.operator_token();

      self
         .children_with_tokens()
         .take_while(|element| {
            let Some(operator_token) = operator_token else {
               // When there is no operator token, take it all.
               return true;
            };

            // Take part before token.
            element
               .into_token()
               .is_none_or(|token| token != operator_token)
         })
         .find_map(|element| <ExpressionRef<'_>>::try_from(element.into_node()?).ok())
   }

   #[must_use]
   pub fn right(&self) -> Option<ExpressionRef<'_>> {
      let operator_token = self.operator_token();

      self
         .children_with_tokens()
         .skip_while(|element| {
            let Some(operator_token) = operator_token else {
               // When there is no operator token, don't skip.
               return false;
            };

            // Skip all until a token, aka the operator.
            element
               .into_token()
               .is_none_or(|token| token != operator_token)
         })
         .filter_map(|element| <ExpressionRef<'_>>::try_from(element.into_node()?).ok())
         .last()
   }

   /// Returns the operator token of this operation.
   pub fn operator_token(&self) -> Option<&'_ red::Token> {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find(|token| InfixOperator::try_from(token.kind()).is_ok())
   }

   /// Returns the operator of this operation.
   pub fn operator(&self) -> InfixOperator {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find_map(|token| InfixOperator::try_from(token.kind()).ok())
         .unwrap_or(InfixOperator::ImplicitCall)
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let expressions = [self.left(), self.right()];

      // TODO: Temporary.
      if expressions.iter().any(Option::is_none) {
         to.push(
            Report::error("curried infix functions aren't supported yet")
               .primary(self.span(), "unsupported")
               .tip("create a lambda instead"),
         );
         return;
      }

      for expression in expressions.iter().flatten() {
         expression.validate(to);
      }

      let operator = self.operator();
      let (InfixOperator::Call | InfixOperator::Pipe) = operator else {
         return;
      };

      for expression in expressions.iter().flatten() {
         if let &ExpressionRef::InfixOperation(operation) = expression
            && let child_operator @ (InfixOperator::Call | InfixOperator::Pipe) =
               operation.operator()
            && child_operator != operator
         {
            to.push(
               Report::error("call and pipe operators do not associate")
                  .secondary(self.span(), "this")
                  .primary(operation.span(), "does not associate with this"),
            );
         }
      }
   }
}

// SUFFIX OPERATION

/// A suffix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuffixOperator {}

impl TryFrom<Kind> for SuffixOperator {
   type Error = ();

   #[expect(clippy::match_single_binding)]
   fn try_from(from: Kind) -> Result<Self, ()> {
      match from {
         _ => Err(()),
      }
   }
}

impl SuffixOperator {
   /// Returns the binding power of this operator.
   #[must_use]
   pub fn binding_power(self) -> (u16, ()) {
      match self {}
   }
}

node! {
   #[from(NODE_SUFFIX_OPERATION)]
   /// A suffix operation.
   struct SuffixOperation;
}

impl SuffixOperation {
   get_node! { left -> 0 @ Option<ExpressionRef<'_>> }

   /// Returns the operator token of this operation.
   pub fn operator_token(&self) -> &'_ red::Token {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find(|token| SuffixOperator::try_from(token.kind()).is_ok())
         .unwrap()
   }

   /// Returns the operator of this operation.
   pub fn operator(&self) -> SuffixOperator {
      self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .find_map(|token| SuffixOperator::try_from(token.kind()).ok())
         .unwrap()
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if let Some(left) = self.left() {
         left.validate(to);
      }
   }
}

// INTERPOLATION

node! {
   #[from(NODE_INTERPOLATION)]
   /// Interpolation. Is a segment that has a single expression within.
   struct Interpolation;
}

impl Interpolation {
   get_token! { interpolation_token_start -> TOKEN_INTERPOLATION_START }

   get_node! { expression -> ExpressionRef<'_> }

   get_token! { interpolation_token_end -> Option<TOKEN_INTERPOLATION_END> }
}

// PATH

node! {
   #[from(NODE_PATH)]
   /// A path.
   struct Path;
}

impl Segmented for Path {}

impl Path {
   pub fn validate(&self, to: &mut Vec<Report>) {
      // No `validate_closing` here because paths are always closed.
      // It has no visible closing delimiter.

      let mut report = lazy!(Report::error("invalid path"));

      let segments = self.segments();
      segments.validate(to, &mut report);

      // Only assert if the report wasn't initialized, because
      // /etc/ssl\<newline-here> actually gets parsed as a
      // multiline segment. And when that happens report is ready.
      if !ready!(report) {
         assert!(!segments.is_multiline);
      }

      if let Some(report) = read!(report) {
         to.push(report);
      }
   }
}

// BIND

node! {
   #[from(NODE_BIND)]
   /// A bind. Contains an identifier to bind to when compared with a value.
   struct Bind;
}

impl Bind {
   get_token! { token_at -> TOKEN_AT }

   get_node! { expression -> ExpressionRef<'_> }

   #[must_use]
   pub fn identifier(&self) -> &Identifier {
      let ExpressionRef::Identifier(identifier) = self.expression() else {
         unreachable!("node must be valid")
      };

      identifier
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let identifier = self.expression();

      if let ExpressionRef::Identifier(identifier) = identifier {
         identifier.validate(to);
      } else if identifier.kind() != NODE_ERROR {
         to.push(Report::error("invalid bind").primary(
            identifier.span(),
            format!(
               "expected an identifier, not {kind}",
               kind = identifier.kind()
            ),
         ));
      }
   }
}

// IDENTIFIER

node! {
   #[from(NODE_IDENTIFIER)]
   /// A quoted identifier.
   struct IdentifierQuoted;
}

impl Segmented for IdentifierQuoted {}

impl IdentifierQuoted {
   pub fn validate(&self, to: &mut Vec<Report>) {
      let to_len = to.len();
      self.validate_closing(to, TOKEN_QUOTED_IDENTIFIER_END, "quoted identifier");
      if to_len != to.len() {
         return;
      }

      let mut report = lazy!(Report::error("invalid quoted identifier"));

      let segments = self.segments();
      segments.validate(to, &mut report);

      if segments.is_multiline {
         force!(report).push_primary(self.span(), "here");
         force!(report).push_tip("quoted identifiers cannot contain newlines");
      }

      if let Some(report) = read!(report) {
         to.push(report);
      }
   }
}

reffed! {
   /// An identifier value.
   #[derive(Debug, Clone, PartialEq, Eq, Hash)]
   pub enum IdentifierValue {
      /// A plain identifier backed by a [`token::Identifier`].
      Plain(token::Identifier),
      /// A quoted identifier backed by a [`IdentifierQuoted`].
      Quoted(IdentifierQuoted),
   }
}

impl IdentifierValueRef<'_> {
   /// Return whether this value can be treated as a literal.
   #[must_use]
   pub fn is_trivial(self) -> bool {
      match self {
         IdentifierValueRef::Plain(_) => true,
         IdentifierValueRef::Quoted(quoted) => quoted.is_trivial(),
      }
   }
}

node! {
   #[from(NODE_IDENTIFIER)]
   /// An identifier. Can either be a raw identifier token or a quoted identifier.
   struct Identifier;
}

impl Identifier {
   /// Returns the value of this identifier. A value may either be a
   /// [`token::Identifier`] or a [`IdentifierQuoted`].
   #[must_use]
   pub fn value(&self) -> IdentifierValueRef<'_> {
      let first_token = self
         .first_token()
         .expect("identifier node must have children");

      assert!(
         !first_token.kind().is_trivia(),
         "identifier node's first child must not be trivia"
      );

      if let Ok(token) = <&token::Identifier>::try_from(first_token) {
         return IdentifierValueRef::Plain(token);
      }

      if let Ok(quoted) = <&IdentifierQuoted>::try_from(&**self) {
         return IdentifierValueRef::Quoted(quoted);
      }

      unreachable!("identifier node must contain an identifier token or quoted identifier")
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if let IdentifierValueRef::Quoted(quoted) = self.value() {
         quoted.validate(to);
      }
   }
}

// STRING

node! {
   #[from(NODE_STRING)]
   /// A string.
   struct SString;
}

impl Segmented for SString {}

impl SString {
   pub fn validate(&self, to: &mut Vec<Report>) {
      let to_len = to.len();
      self.validate_closing(to, TOKEN_STRING_END, "string");
      if to_len != to.len() {
         return;
      }

      let mut report = lazy!(Report::error("invalid string"));

      let segments = self.segments();
      segments.validate(to, &mut report);

      if let Some(report) = read!(report) {
         to.push(report);
      }
   }
}

// CHAR

node! {
   #[from(NODE_CHAR)]
   /// A character.
   struct Char;
}

impl Segmented for Char {}

impl Char {
   #[must_use]
   pub fn value(&self) -> char {
      let Segment::Content { content, .. } = self.segments().into_iter().next().unwrap() else {
         unreachable!()
      };

      content.chars().next().unwrap()
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let to_len = to.len();
      self.validate_closing(to, TOKEN_CHAR_END, "char");
      if to_len != to.len() {
         return;
      }

      let mut report = lazy!(Report::error("invalid char"));

      let segments = self.segments();
      segments.validate(to, &mut report);

      if segments.is_multiline {
         force!(report).push_primary(self.span(), "chars cannot cannot contain newlines");
      }

      if !ready!(report) {
         let mut got: usize = 0;
         for segment in segments {
            match segment {
               Segment::Content { content, .. } => {
                  got += content.chars().count();
               },

               Segment::Interpolation(interpolation) => {
                  force!(report)
                     .push_primary(interpolation.span(), "chars cannot contain interpolation");
               },
            }
         }

         match got {
            0 => force!(report).push_primary(self.span(), "empty char"),
            1 => {},
            _ => force!(report).push_primary(self.span(), "too long"),
         }
      }

      if let Some(report) = read!(report) {
         to.push(report);
      }
   }
}

// INTEGER

node! {
   #[from(NODE_INTEGER)]
   /// An integer.
   struct Integer;
}

impl Integer {
   get_token! { token_integer -> &token::Integer }

   #[must_use]
   pub fn value(&self) -> num::BigInt {
      self.token_integer().value().expect("integer must be valid")
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if self.token_integer().value().is_err() {
         to.push(
            Report::error("invalid integer").primary(self.span(), "why do you even need this?"),
         );
      }
   }
}

// FLOAT

node! {
   #[from(NODE_FLOAT)]
   /// A float.
   struct Float;
}

impl Float {
   get_token! { token_float -> &token::Float }

   #[must_use]
   pub fn value(&self) -> f64 {
      self.token_float().value().expect("float must be valid")
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      if self.token_float().value().is_err() {
         to.push(Report::error("invalid float").primary(self.span(), "usecase?"));
      }
   }
}

// IF

node! {
   #[from(NODE_IF)]
   /// An if-else.
   struct If;
}

impl If {
   get_token! { token_if -> TOKEN_KEYWORD_IF }

   get_node! { condition -> 0 @ ExpressionRef<'_> }

   get_token! { token_then -> Option<TOKEN_KEYWORD_THEN> }

   get_node! { consequence -> 1 @ ExpressionRef<'_> }

   get_token! { token_else -> Option<TOKEN_KEYWORD_ELSE> }

   get_node! { alternative -> 2 @ ExpressionRef<'_> }

   pub fn validate(&self, to: &mut Vec<Report>) {
      self.condition().validate(to);
      self.consequence().validate(to);
      self.alternative().validate(to);
   }
}

mod segment {
   // For the next poor soul that will step in this module:
   //
   // Beware that changing even the slighest thing will break 500 other cases. Way
   // too many hours have been spent on perfecting this, and every single invariant
   // is (probably) intended. Please reconsider editing this file.
   //
   // Comments? Ha!

   use std::{
      mem,
      ops,
   };

   use cab_util::{
      Lazy,
      force_ref,
      reffed,
   };
   use ranged::{
      IntoSize as _,
      IntoSpan as _,
      Span,
   };
   use smallvec::SmallVec;
   use ust::report::Report;

   use crate::{
      Kind,
      node,
      red,
      token,
   };

   reffed! {
      #[derive(Debug, Clone, PartialEq, Eq, Hash)]
      enum SegmentRaw {
         Content(token::Content),
         Interpolation(node::Interpolation),
      }
   }

   impl SegmentRawRef<'_> {
      #[must_use]
      fn span_first_line(self) -> Span {
         match self {
            SegmentRawRef::Content(content) => {
               match content.text().find('\n') {
                  Some(len) => Span::at(content.span().start, len),
                  None => content.span(),
               }
            },

            SegmentRawRef::Interpolation(interpolation) => {
               match interpolation.text().find_char('\n') {
                  Some(len) => Span::at(interpolation.span().start, len),
                  None => interpolation.span(),
               }
            },
         }
      }

      #[must_use]
      fn span_last_line(self) -> Span {
         match self {
            SegmentRawRef::Content(content) => {
               match content.text().rfind('\n') {
                  Some(len) => {
                     Span::at_end(
                        content.span().end,
                        content.text().size() - len - '\n'.size(),
                     )
                  },
                  None => content.span(),
               }
            },

            SegmentRawRef::Interpolation(interpolation) => {
               match interpolation.text().rfind_char('\n') {
                  Some(len) => {
                     Span::at_end(
                        interpolation.span().end,
                        interpolation.text().size() - len - '\n'.size(),
                     )
                  },
                  None => interpolation.span(),
               }
            },
         }
      }
   }

   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum Segment<'a> {
      Content { span: Span, content: String },
      Interpolation(&'a node::Interpolation),
   }

   impl Segment<'_> {
      #[must_use]
      pub fn is_content(&self) -> bool {
         matches!(self, &Self::Content { .. })
      }

      #[must_use]
      pub fn is_interpolation(&self) -> bool {
         matches!(self, &Self::Interpolation(_))
      }
   }

   #[derive(Debug, Clone, PartialEq, Eq)]
   enum Straight<'a> {
      Line {
         span: Span,
         text: &'a str,

         is_from_line_start: bool,
         is_to_line_end:     bool,

         is_first: bool,
         is_last:  bool,
      },

      Interpolation(&'a node::Interpolation),
   }

   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct Segments<'a> {
      span: Span,

      pub is_multiline: bool,

      line_span_first: Option<Span>,
      line_span_last:  Option<Span>,

      straights: SmallVec<Straight<'a>, 4>,
   }

   impl<'a> IntoIterator for Segments<'a> {
      type Item = Segment<'a>;

      type IntoIter = impl Iterator<Item = Segment<'a>>;

      fn into_iter(self) -> Self::IntoIter {
         gen move {
            let mut buffer = String::new();
            let mut buffer_span = None::<Span>;

            let (indent, indent_width) = self
               .indent()
               .expect("string must be valid and not mix indents");

            for straight in self.straights {
               match straight {
                  Straight::Line {
                     span,
                     mut text,
                     is_from_line_start,
                     is_to_line_end,
                     is_first,
                     is_last,
                  } => {
                     if self.is_multiline {
                        // Multiline strings' first and last lines are ignored:
                        //
                        // "<ignored>
                        // <content>
                        // <ignored>"
                        if is_first || is_last {
                           assert!(
                              text.chars().all(char::is_whitespace),
                              "multiline string must be valid and not have non-whitespace \
                               characters in first and last lines"
                           );
                           continue;
                        }

                        if is_to_line_end {
                           text = text.trim_end();
                        }

                        if is_from_line_start {
                           text = if text.chars().all(char::is_whitespace) {
                              ""
                           } else {
                              assert!(
                                 text[..indent_width].chars().all(|c| c == indent.unwrap()),
                                 "multiline string must be valid and not mix indents"
                              );
                              &text[indent_width..]
                           }
                        }
                     }

                     let (unescaped, escaped_newline) =
                        token::unescape_string(text).expect("string content must be valid");

                     buffer.push_str(&unescaped);

                     // Not asserting `escaped_newline -> is_to_line_end`,
                     // because we still process invalid syntax and
                     // yield valid segments.
                     //
                     // For example, in this code:
                     //
                     //   "\
                     //
                     // That part with only a \ will `escaped_newline`, but
                     // it won't be a `is_to_line_end` because the way
                     // we decide that is just `!line_is_last`, which is false
                     // as that "line" is the last as there is no closing delimiter.
                     //
                     // That's fine for actually valid syntax trees though.

                     if is_to_line_end && !escaped_newline {
                        buffer.push('\n');
                     }

                     buffer_span.replace(buffer_span.map_or(span, |span_| span_.cover(span)));
                  },

                  Straight::Interpolation(interpolation) => {
                     yield Segment::Content {
                        span:    buffer_span
                           .take()
                           .expect("interpolation must never be the first or last segment"),
                        content: mem::take(&mut buffer),
                     };

                     yield Segment::Interpolation(interpolation);
                  },
               }
            }

            if let Some(span) = buffer_span {
               yield Segment::Content {
                  span,
                  content: buffer,
               };
            }
         }
      }
   }

   impl Segments<'_> {
      fn indent(&self) -> Result<(Option<char>, usize), SmallVec<char, 4>> {
         let mut indents = SmallVec::<char, 4>::new();
         let mut indent_width = None::<usize>;

         for straight in &self.straights {
            let &Straight::Line {
               text,
               is_from_line_start: true,
               is_last: false,
               ..
            } = straight
            else {
               continue;
            };

            if text.chars().all(char::is_whitespace) {
               continue;
            }

            let mut line_indent_width: usize = 0;

            for c in text.chars() {
               if !c.is_whitespace() {
                  break;
               }

               line_indent_width += 1;

               if !indents.contains(&c) {
                  indents.push(c);
               }
            }

            if let Some(width) = indent_width {
               indent_width.replace(width.min(line_indent_width));
            } else {
               indent_width.replace(line_indent_width);
            }
         }

         if indents.len() > 1 {
            return Err(indents);
         }

         Ok((indents.first().copied(), indent_width.unwrap_or(0)))
      }

      pub fn validate(&self, to: &mut Vec<Report>, report: &mut Lazy!(Report)) {
         for straight in &self.straights {
            match *straight {
               Straight::Line { span, text, .. } => {
                  if let Err(invalids) = token::unescape_string(text) {
                     for invalid in invalids {
                        force_ref!(report)
                           .push_primary(invalid.offset(span.start), "invalid escape");
                     }
                  }
               },

               Straight::Interpolation(interpolation) => interpolation.expression().validate(to),
            }
         }

         if let Err(indents) = self.indent() {
            force_ref!(report).push_primary(
               self.span,
               format!(
                  "cannot mix different kinds of space in indents: {indents}",
                  indents = indents
                     .into_iter()
                     .map(|c| {
                        match token::escape(c)
                           .is_first(true)
                           .delimiter(('\'', "\\'"))
                           .call()
                        {
                           Some(escaped) => escaped.to_owned(),
                           None => format!("'{c}'"),
                        }
                     })
                     .intersperse(", ".to_owned())
                     .collect::<String>(),
               ),
            );
         }

         if self.is_multiline {
            for span in [self.line_span_first, self.line_span_last]
               .into_iter()
               .flatten()
            {
               force_ref!(report).push_primary(span, "first and last lines must be empty");
            }
         }
      }
   }

   pub trait Segmented: ops::Deref<Target = red::Node> {
      fn segments(&self) -> Segments<'_> {
         let mut is_multiline = false;

         let mut line_span_first = None::<Span>;
         let mut line_span_last = None::<Span>;

         let mut straights = SmallVec::new();

         let mut previous_segment_span_last_line = None::<Span>;
         let mut segments = self
            .children_with_tokens()
            .filter_map(|child| {
               match child {
                  red::ElementRef::Node(node) => {
                     Some(SegmentRawRef::Interpolation(
                        <&node::Interpolation>::try_from(node)
                           .expect("child node of segmented node must be interpolation"),
                     ))
                  },

                  // The reason we are not asserting here is because invalid
                  // segmented nodes sometimes contain non-content tokens,
                  // it's not worth it to fix this as it'll error anyway.
                  red::ElementRef::Token(token) => {
                     <&token::Content>::try_from(token)
                        .map(SegmentRawRef::Content)
                        .ok()
                  },
               }
            })
            .enumerate()
            .peekable();

         while let Some((segment_index, segment)) = segments.next() {
            let mut segment_is_multiline = false;

            let segment_is_first = segment_index == 0;
            let segment_is_last = segments.peek().is_none();

            match segment {
               SegmentRawRef::Content(content) => {
                  let span = content.span();

                  let mut offset: usize = 0;
                  let mut lines = content.text().split('\n').enumerate().peekable();
                  while let Some((line_index, line)) = lines.next() {
                     let line_is_first = line_index == 0;
                     let line_is_last = lines.peek().is_none();

                     if line_is_first && !line_is_last {
                        segment_is_multiline = true;
                     }

                     if segment_is_first && line_is_first {
                        let suffix_interpolation_span = line_is_last
                           .then(|| {
                              segments
                                 .peek()
                                 .map(|&(_, segment)| segment.span_first_line())
                           })
                           .flatten();

                        if let Some(interpolation_span) = suffix_interpolation_span {
                           line_span_first.replace(span.cover(interpolation_span));
                        } else {
                           let line = line.trim_end();

                           if !line.is_empty() {
                              line_span_first.replace(Span::at(span.start, line.size()));
                           }
                        }
                     }

                     if segment_is_last && line_is_last {
                        let prefix_interpolation_span_last_line = line_is_first
                           .then_some(previous_segment_span_last_line)
                           .flatten();

                        if let Some(interpolation_span_last_line) =
                           prefix_interpolation_span_last_line
                        {
                           line_span_last.replace(span.cover(interpolation_span_last_line));
                        } else {
                           let line = line.trim_start();

                           if !line.is_empty() {
                              line_span_last.replace(Span::at_end(span.end, line.size()));
                           }
                        }
                     }

                     #[expect(clippy::nonminimal_bool)]
                     straights.push(Straight::Line {
                        span: Span::at(content.span().start + offset, line.size()),

                        text: &content.text()[offset..offset + line.len()],

                        is_from_line_start: !(segment_is_first && line_is_first)
                           && !(previous_segment_span_last_line.is_some() && line_is_first),
                        is_to_line_end:     !line_is_last,

                        is_first: segment_is_first && line_is_first,
                        is_last:  segment_is_last && line_is_last,
                     });

                     offset += line.len() + '\n'.len_utf8();
                  }
               },

               SegmentRawRef::Interpolation(interpolation) => {
                  let span = interpolation.span();

                  if segment_is_first {
                     line_span_first.replace(span);
                  }

                  if segment_is_last {
                     line_span_last.replace(span);
                  }

                  straights.push(Straight::Interpolation(interpolation));
               },
            }

            previous_segment_span_last_line.replace(segment.span_last_line());

            if segment_is_multiline {
               is_multiline = true;
            }
         }

         Segments {
            span: self.span(),

            is_multiline,

            line_span_first,
            line_span_last,

            straights,
         }
      }

      fn is_trivial(&self) -> bool {
         let mut segments = self.segments().into_iter().peekable();

         segments.next().is_some_and(|segment| segment.is_content()) && segments.peek().is_none()
      }

      fn validate_closing(&self, to: &mut Vec<Report>, end: Kind, type_: &str) {
         if self
            .children_with_tokens()
            .last()
            .is_some_and(|token| token.kind() == end)
         {
            return;
         }

         let start = self
            .children_with_tokens()
            .next()
            .expect("delimited must have tokens");

         to.push(
            Report::error(format!("unclosed {type_}"))
               .secondary(start.span(), format!("{type_} starts here"))
               .primary(Span::empty(self.span().end), format!("expected {end} here")),
         );
      }
   }
}
