use std::ops;

use cab_why::Span;

use crate::{
   Operation,
   Value,
};

const ENCODED_SIZE_U64: usize = 9;
const ENCODED_SIZE_U16: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteIndex(usize);

impl ops::Deref for ByteIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValueIndex(usize);

impl ops::Deref for ValueIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

pub struct Code {
   content: Vec<u8>,
   spans:   Vec<(ByteIndex, Span)>,

   values: Vec<Value>,
}

impl Code {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self {
         content: Vec::new(),
         spans:   Vec::new(),
         values:  Vec::new(),
      }
   }

   pub fn push_u64(&mut self, data: u64) -> ByteIndex {
      let mut encoded = [0; ENCODED_SIZE_U64];
      let len = vu128::encode_u64(&mut encoded, data);

      let index = ByteIndex(self.content.len());
      self.content.extend_from_slice(&encoded[..len]);
      index
   }

   #[must_use]
   pub fn read_u64(&self, index: ByteIndex) -> (u64, usize) {
      let encoded = match self.content.get(*index..*index + ENCODED_SIZE_U64) {
         Some(slice) => slice.try_into().expect("size was statically checked"),

         None => {
            let mut buffer = [0; ENCODED_SIZE_U64];
            buffer[..self.content.len() - *index].copy_from_slice(
               self
                  .content
                  .get(*index..)
                  .expect("cab-runtime bug: invalid byte index"),
            );
            buffer
         },
      };

      vu128::decode_u64(&encoded)
   }

   pub fn push_u16(&mut self, data: u16) -> ByteIndex {
      let index = ByteIndex(self.content.len());
      self.content.extend_from_slice(&data.to_le_bytes());
      index
   }

   #[must_use]
   pub fn read_u16(&self, index: ByteIndex) -> (u16, usize) {
      let encoded = self
         .content
         .get(*index..*index + ENCODED_SIZE_U16)
         .expect("cab-runtime bug: invalid byte index")
         .try_into()
         .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_SIZE_U16)
   }

   pub fn push_operation(&mut self, span: Span, operation: Operation) -> ByteIndex {
      let index = ByteIndex(self.content.len());
      self.content.push(operation as u8);

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
   pub fn read_operation(&self, index: ByteIndex) -> (Span, Operation) {
      let position = self.spans.partition_point(|&(index2, _)| index >= index2);

      let (index, span) = self.spans[position.saturating_sub(1)];

      (
         span,
         self.content[*index]
            .try_into()
            .expect("cab-runtime bug: invalid operation at byte index"),
      )
   }

   // TODO: Maybe return ByteIndex?
   pub fn push_value(&mut self, span: Span, value: Value) -> ValueIndex {
      let index = ValueIndex(self.values.len());
      self.values.push(value);

      self.push_operation(span, Operation::Value);
      self.push_u64(*index as _);

      index
   }

   // TODO: Maybe require ByteIndex?
   #[must_use]
   pub fn read_value(&self, index: ValueIndex) -> &Value {
      self
         .values
         .get(*index)
         .expect("cab-runtime bug: invalid value index")
   }

   pub fn set_here(&mut self, index: ByteIndex) {
      let here = self.content.len() as u16;

      self.content[*index..*index + ENCODED_SIZE_U16].copy_from_slice(&here.to_le_bytes());
   }
}
