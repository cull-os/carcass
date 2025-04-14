use std::ops;

use cab_why::Span;

use crate::{
   Operation,
   Value,
};

const ENCODED_U64_SIZE: usize = 9;
const ENCODED_U16_SIZE: usize = 2;

#[derive(Debug, Clone, Copy)]
pub struct ByteIndex(usize);

impl ops::Deref for ByteIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

#[derive(Debug, Clone, Copy)]
pub struct ConstantIndex(usize);

impl ops::Deref for ConstantIndex {
   type Target = usize;

   fn deref(&self) -> &Self::Target {
      &self.0
   }
}

pub struct Code {
   content: Vec<u8>,
   spans:   Vec<(ByteIndex, Span)>,

   constants: Vec<Value>,
}

impl Code {
   pub fn push_u64(&mut self, data: u64) -> ByteIndex {
      let mut encoded = [0; ENCODED_U64_SIZE];
      let len = vu128::encode_u64(&mut encoded, data);

      let id = ByteIndex(self.content.len());
      self.content.extend_from_slice(&encoded[..len]);
      id
   }

   pub fn read_u64(&self, id: ByteIndex) -> (u64, usize) {
      let encoded = match self.content.get(*id..*id + ENCODED_U64_SIZE) {
         Some(slice) => slice.try_into().expect("size was statically checked"),

         None => {
            let mut buffer = [0; ENCODED_U64_SIZE];
            buffer[..self.content.len() - *id].copy_from_slice(
               self
                  .content
                  .get(*id..)
                  .expect("cab-runtime bug: invalid code id"),
            );
            buffer
         },
      };

      vu128::decode_u64(&encoded)
   }

   pub fn push_u16(&mut self, data: u16) -> ByteIndex {
      let id = ByteIndex(self.content.len());
      self.content.extend_from_slice(&data.to_le_bytes());
      id
   }

   pub fn read_u16(&self, id: ByteIndex) -> (u16, usize) {
      let encoded = self
         .content
         .get(*id..*id + ENCODED_U16_SIZE)
         .expect("cab-runtime bug: invalid code id")
         .try_into()
         .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_U16_SIZE)
   }

   pub fn push_constant(&mut self, value: Value) -> ConstantIndex {
      let id = self.constants.len();
      self.constants.push(value);
      ConstantIndex(id)
   }

   pub fn read_constant(&self, id: ConstantIndex) -> &Value {
      self
         .constants
         .get(*id)
         .expect("cab-runtime bug: invalid constant id")
   }

   pub fn push_operation(&mut self, span: Span, operation: Operation) -> ByteIndex {
      let id = ByteIndex(self.content.len());
      self.content.push(operation as u8);

      // No need to insert the span again if this instruction was created from the
      // last span.
      if self
         .spans
         .last()
         .is_none_or(|&(_, last_span)| last_span != span)
      {
         self.spans.push((id, span));
      }

      id
   }

   pub fn read_operation(&self, id: ByteIndex) -> (Span, Operation) {
      let position = self.spans.binary_search_by(|(id2, _)| id2.cmp(&id));

      let (id, span) = match position {
         Ok(index) => self.spans[index],
         Err(0) => self.spans[0],
         Err(index) => self.spans[index - 1],
      };

      (
         span,
         self.content[*id]
            .try_into()
            .expect("cab-runtime bug: invalid operation at code id"),
      )
   }
}
