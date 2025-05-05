//! Typed [`Node`] definitions.
//!
//! [`Node`]: crate::Node
use std::{
   collections::VecDeque,
   ops,
   ptr,
};

use cab_report::Report;
use cab_span::{
   IntoSpan as _,
   Span,
};
use cab_util::{
   force,
   lazy,
   read,
   reffed,
};
use derive_more::Deref;
use paste::paste;

use crate::{
   Kind::{
      self,
      *,
   },
   red,
   segment::{
      Segment,
      Segmented,
   },
   token,
};

macro_rules! node {
   (
      #[from($kind:ident)]
      $(#[$attribute:meta])*
      struct $name:ident;
   ) => {
      $(#[$attribute])*
      #[derive(Deref, Debug, Clone, PartialEq, Eq, Hash)]
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
         #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

      Rune,
      Integer,
      Float,

      If,
   )]
   /// An expression. Everything is an expression.
   enum Expression;
}

impl<'a> ExpressionRef<'a> {
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
         Self::Rune(rune) => rune.validate(to),
         Self::If(if_else) => if_else.validate(to),

         Self::Error(_) | Self::Integer(_) | Self::Float(_) => {},
      }
   }

   /// Iterates over all subexpressions delimited with the same operator.
   #[expect(irrefutable_let_patterns)]
   pub fn same_items(self) -> impl Iterator<Item = ExpressionRef<'a>> {
      gen move {
         let mut expressions = VecDeque::from([self]);

         while let Some(expression) = expressions.pop_back() {
            match expression {
               ExpressionRef::InfixOperation(operation)
                  if let InfixOperator::Same = operation.operator() =>
               {
                  expressions.push_front(operation.left());
                  expressions.push_front(operation.right());
               },

               ExpressionRef::SuffixOperation(operation)
                  if let SuffixOperator::Same = operation.operator() =>
               {
                  expressions.push_front(operation.left());
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
   get_node! { right -> 0 @ ExpressionRef<'_> }

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
      self.right().validate(to);
   }
}

// INFIX OPERATION

/// An infix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfixOperator {
   Same,
   Sequence,

   ImplicitApply,
   Apply,
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

         kind if kind.is_argument() => Self::ImplicitApply,
         TOKEN_LESS_PIPE => Self::Apply,
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
            Self::ImplicitApply => (170, 175),

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
            Self::Apply => (55, 50),

            Self::Lambda => (45, 40),

            Self::Equal | Self::NotEqual => (35, 30),

            Self::Same => (25, 20),
            Self::Sequence => (15, 10),
        }
   }

   /// Whether if this operator actually owns a token. Not owning a token means
   /// that the operator doesn't actually "exist".
   #[must_use]
   pub fn is_token_owning(self) -> bool {
      self != Self::ImplicitApply
   }
}

node! {
   #[from(NODE_INFIX_OPERATION)]
   /// An infix operation.
   struct InfixOperation;
}

impl InfixOperation {
   get_node! { left -> 0 @ ExpressionRef<'_> }

   get_node! { right -> 1 @ ExpressionRef<'_> }

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
         .unwrap_or(InfixOperator::ImplicitApply)
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let expressions = &[self.left(), self.right()];

      for expression in expressions {
         expression.validate(to);
      }

      let operator = self.operator();
      let (InfixOperator::Apply | InfixOperator::Pipe) = operator else {
         return;
      };

      for expression in expressions {
         if let &ExpressionRef::InfixOperation(operation) = expression
            && let child_operator @ (InfixOperator::Apply | InfixOperator::Pipe) =
               operation.operator()
            && child_operator != operator
         {
            to.push(
               Report::error("application and pipe operators do not associate")
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
pub enum SuffixOperator {
   Same,
}

impl TryFrom<Kind> for SuffixOperator {
   type Error = ();

   fn try_from(from: Kind) -> Result<Self, ()> {
      match from {
         TOKEN_COMMA => Ok(Self::Same),

         _ => Err(()),
      }
   }
}

node! {
   #[from(NODE_SUFFIX_OPERATION)]
   /// A suffix operation.
   struct SuffixOperation;
}

impl SuffixOperation {
   get_node! { left -> 0 @ ExpressionRef<'_> }

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
      self.left().validate(to);
   }
}

// INTERPOLATION

node! {
   #[from(NODE_INTERPOLATION)]
   /// Interpolation. Is a content part that has a single expression within.
   struct Interpolation;
}

impl Interpolation {
   get_token! { interpolation_token_start -> TOKEN_INTERPOLATION_START }

   get_node! { expression -> ExpressionRef<'_> }

   get_token! { interpolation_token_end -> Option<TOKEN_INTERPOLATION_END> }
}

// PATH

node! {
   #[from(NODE_PATH_ROOT_TYPE)]
   /// A path root type.
   struct PathRootType;
}

impl Segmented for PathRootType {}

impl PathRootType {
   get_token! { token_delimiter_left -> TOKEN_LESS }

   pub fn token_delimiter_right(&self) -> Option<&red::Token> {
      let token = self
         .children_with_tokens()
         .filter_map(red::ElementRef::into_token)
         .last();

      if let Some(token) = token {
         assert!((TOKEN_MORE | TOKEN_COLON).contains(token.kind()));
      }

      token
   }
}

node! {
   #[from(NODE_PATH_ROOT)]
   /// A path root.
   struct PathRoot;
}

impl PathRoot {
   #[must_use]
   pub fn token_delimiter_left(&self) -> &red::Token {
      self.type_().token_delimiter_left()
   }

   get_node! { type_ -> &PathRootType }

   pub fn config(&self) -> Option<ExpressionRef<'_>> {
      // Right after the header, must be a node.
      self
         .children_with_tokens()
         .nth(1)
         .and_then(red::ElementRef::into_node)
         .and_then(|node| ExpressionRef::try_from(node).ok())
   }

   get_token! { token_colon -> Option<TOKEN_COLON> }

   pub fn path(&self) -> Option<ExpressionRef<'_>> {
      self
         .children_with_tokens()
         .skip_while(|element| {
            element
               .into_token()
               .is_none_or(|token| token.kind() != TOKEN_COLON)
         })
         .nth(1)
         .and_then(red::ElementRef::into_node)
         .and_then(|node| ExpressionRef::try_from(node).ok())
   }

   pub fn token_delimiter_right(&self) -> Option<&red::Token> {
      let header_delimiter_right = self.type_().token_delimiter_right();

      let token = if header_delimiter_right.is_none_or(|token| token.kind() == TOKEN_MORE) {
         header_delimiter_right
      } else {
         self
            .children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .last()
      };

      if let Some(token) = token {
         assert_eq!(token.kind(), TOKEN_MORE);
      }

      token
   }
}

node! {
   #[from(NODE_PATH_CONTENT)]
   /// A path content.
   struct PathContent;
}

impl Segmented for PathContent {}

node! {
   #[from(NODE_PATH)]
   /// A path.
   struct Path;
}

impl Path {
   get_node! { root -> Option<&PathRoot> }

   get_node! { content -> Option<&PathContent> }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let mut report = lazy!(Report::error("invalid path root"));

      if let Some(root) = self.root() {
         let segments = root.type_().segments();
         segments.validate(&mut report, to);

         if segments.is_multiline {
            force!(report).push_primary(root.type_().span(), "here");
            force!(report).push_tip("path roots cannot contain newlines");
         }

         if let Some(config) = root.config() {
            config.validate(to);
         }

         if let Some(path) = root.path() {
            path.validate(to);
         }
      }

      if let Some(content) = self.content() {
         content.segments().validate(&mut report, to);
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

   get_node! { identifier -> ExpressionRef<'_> }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let identifier = self.identifier();

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
      let mut report = lazy!(Report::error("invalid identifier"));

      let segments = self.segments();
      segments.validate(&mut report, to);

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
      let Some(first_token) = self.first_token() else {
         unreachable!()
      };

      assert!(!first_token.kind().is_trivia());

      if let Ok(token) = <&token::Identifier>::try_from(first_token) {
         return IdentifierValueRef::Plain(token);
      }

      if let Ok(quoted) = <&IdentifierQuoted>::try_from(&**self) {
         return IdentifierValueRef::Quoted(quoted);
      }

      panic!("identifier node did not have an identifier or identifier starter token")
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
      let mut report = lazy!(Report::error("invalid string"));

      let segments = self.segments();
      segments.validate(&mut report, to);

      if let Some(report) = read!(report) {
         to.push(report);
      }
   }
}

// RUNE

node! {
   #[from(NODE_RUNE)]
   /// A rune. Also known as a character.
   struct Rune;
}

impl Segmented for Rune {}

impl Rune {
   #[must_use]
   pub fn value(&self) -> char {
      let Segment::Content { content, .. } = self.segments().into_iter().next().unwrap() else {
         unreachable!()
      };

      content.chars().next().unwrap()
   }

   pub fn validate(&self, to: &mut Vec<Report>) {
      let mut report = lazy!(Report::error("invalid rune"));

      let segments = self.segments();
      segments.validate(&mut report, to);

      if segments.is_multiline {
         force!(report).push_primary(self.span(), "runes cannot cannot contain newlines");
      }

      let mut got: usize = 0;
      for segment in segments {
         match segment {
            Segment::Content { content, .. } => {
               got += content.chars().count();
            },

            Segment::Interpolation(interpolation) => {
               force!(report)
                  .push_primary(interpolation.span(), "runes cannot contain interpolation");
            },
         }
      }

      match got {
         0 => force!(report).push_primary(self.span(), "empty rune"),
         1 => {},
         _ => force!(report).push_primary(self.span(), "too long"),
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
      self.token_integer().value()
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
      self.token_float().value()
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
