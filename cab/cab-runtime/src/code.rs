use std::{
   cell::RefCell,
   fmt::{
      self,
      Write as _,
   },
};

use cab_format::{
   indent,
   style::{
      self,
      DOT,
      TOP_TO_BOTTOM,
   },
};
use cab_span::Span;
use derive_more::Deref;

use crate::{
   Argument,
   Operation,
   Value,
};

const ENCODED_U64_LEN: usize = 9;
const ENCODED_U16_LEN: usize = 0u16.to_le_bytes().len();
const ENCODED_OPERATION_LEN: usize = 1;

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteIndex(usize);

impl ByteIndex {
   pub fn dummy() -> Self {
      Self(usize::MAX)
   }
}

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValueIndex(usize);

impl ValueIndex {
   pub fn dummy() -> Self {
      Self(usize::MAX)
   }
}

pub struct Code {
   bytes: Vec<u8>,
   spans: Vec<(ByteIndex, Span)>,

   values: Vec<Value>,
}

impl fmt::Display for Code {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      let index_width = format!("{index:#X}", index = self.bytes.len() - 1).len();

      let index = RefCell::new(ByteIndex(0));
      let mut index_previous = None::<usize>;
      indent!(
         writer,
         index_width + 3,
         with = |writer: &mut dyn fmt::Write| {
            let index = **index.borrow();

            style::GUTTER.fmt_prefix(writer)?;

            if index_previous == Some(index) {
               let dot_width = format!("{index:#X}").len();
               let space_width = index_width - dot_width;

               write!(writer, "{:>space_width$}", "")?;

               for _ in 0..dot_width {
                  write!(writer, "{DOT}")?;
               }
            } else {
               write!(writer, "{index:>#index_width$X}")?;
            }

            write!(writer, " {TOP_TO_BOTTOM} ",)?;
            style::GUTTER.fmt_suffix(writer)?;

            index_previous.replace(index);
            Ok(index_width + 3)
         }
      );

      while **index.borrow() < self.bytes.len() {
         let (_, operation, size) = self.read_operation(*index.borrow());

         write!(writer, "{operation:?}")?;
         index.borrow_mut().0 += size;

         let mut arguments = operation.arguments().iter().enumerate().peekable();
         while let Some((argument_index, argument)) = arguments.next() {
            if argument_index == 0 {
               write!(writer, "(")?;
            }

            let (argument, size) = match argument {
               Argument::U64 => self.read_u64(*index.borrow()),
               Argument::ValueIndex => {
                  let (u64, size) = self.read_u64(*index.borrow());
                  let _ = &self.values[u64 as usize]; // TODO: Proper value printing.
                  (u64 as _, size)
               },

               Argument::U16 => {
                  let (u16, size) = self.read_u16(*index.borrow());
                  (u16 as _, size)
               },
               // TODO: Highlight these properly.
               Argument::ByteIndex => {
                  let (u16, size) = self.read_u16(*index.borrow());
                  (u16 as _, size)
               },
            };

            write!(writer, "{argument}")?;
            index.borrow_mut().0 += size;

            if arguments.peek().is_none() {
               write!(writer, ")")?;
            }
         }

         writeln!(writer)?;
      }

      Ok(())
   }
}

impl Code {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self {
         bytes:  Vec::new(),
         spans:  Vec::new(),
         values: Vec::new(),
      }
   }

   pub fn push_u64(&mut self, data: u64) -> ByteIndex {
      let mut encoded = [0; ENCODED_U64_LEN];
      let len = vu128::encode_u64(&mut encoded, data);

      let index = ByteIndex(self.bytes.len());
      self.bytes.extend_from_slice(&encoded[..len]);
      index
   }

   #[must_use]
   pub fn read_u64(&self, index: ByteIndex) -> (u64, usize) {
      let encoded = match self.bytes.get(*index..*index + ENCODED_U64_LEN) {
         Some(slice) => slice.try_into().expect("size was statically checked"),

         None => {
            let mut buffer = [0; ENCODED_U64_LEN];
            buffer[..self.bytes.len() - *index].copy_from_slice(
               self
                  .bytes
                  .get(*index..)
                  .expect("cab-runtime bug: invalid byte index"),
            );
            buffer
         },
      };

      vu128::decode_u64(&encoded)
   }

   pub fn push_u16(&mut self, data: u16) -> ByteIndex {
      let index = ByteIndex(self.bytes.len());
      self.bytes.extend_from_slice(&data.to_le_bytes());
      index
   }

   #[must_use]
   pub fn read_u16(&self, index: ByteIndex) -> (u16, usize) {
      let encoded = self
         .bytes
         .get(*index..*index + ENCODED_U16_LEN)
         .expect("cab-runtime bug: invalid byte index")
         .try_into()
         .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_U16_LEN)
   }

   pub fn push_operation(&mut self, span: Span, operation: Operation) -> ByteIndex {
      let index = ByteIndex(self.bytes.len());
      self.bytes.push(operation as u8);

      // No need to insert the span again if this instruction was created from the
      // last span.
      if self
         .spans
         .last()
         .is_none_or(|&(_, last_span)| last_span != span)
      {
         self.spans.push((index, span));
      }

      index
   }

   #[must_use]
   pub fn read_operation(&self, index: ByteIndex) -> (Span, Operation, usize) {
      let position = self.spans.partition_point(|&(index2, _)| index >= index2);

      let (_, span) = self.spans[position.saturating_sub(1)];

      (
         span,
         self.bytes[*index]
            .try_into()
            .expect("cab-runtime bug: invalid operation at byte index"),
         ENCODED_OPERATION_LEN,
      )
   }

   pub fn push_value(&mut self, value: Value) -> ValueIndex {
      let index = ValueIndex(self.values.len());
      self.values.push(value);
      index
   }

   #[must_use]
   pub fn read_value(&self, index: ValueIndex) -> &Value {
      self
         .values
         .get(*index)
         .expect("cab-runtime bug: invalid value index")
   }

   pub fn point_here(&mut self, index: ByteIndex) {
      let here = self.bytes.len() as u16;

      self.bytes[*index..*index + ENCODED_U16_LEN].copy_from_slice(&here.to_le_bytes());
   }
}
