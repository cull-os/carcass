use std::ops;

use cab_why::Span;

use crate::{
   Constant,
   Operation,
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

   constants: Vec<Constant>,
}

impl Code {
   #[allow(clippy::new_without_default)]
   pub fn new() -> Self {
      Self {
         content:   Vec::new(),
         spans:     Vec::new(),
         constants: Vec::new(),
      }
   }

   pub fn push_u64(&mut self, data: u64) -> ByteIndex {
      let mut encoded = [0; ENCODED_U64_SIZE];
      let len = vu128::encode_u64(&mut encoded, data);

      let index = ByteIndex(self.content.len());
      self.content.extend_from_slice(&encoded[..len]);
      index
   }

   #[must_use]
   pub fn read_u64(&self, index: ByteIndex) -> (u64, usize) {
      let encoded = match self.content.get(*index..*index + ENCODED_U64_SIZE) {
         Some(slice) => slice.try_into().expect("size was statically checked"),

         None => {
            let mut buffer = [0; ENCODED_U64_SIZE];
            buffer[..self.content.len() - *index].copy_from_slice(
               self
                  .content
                  .get(*index..)
                  .expect("cab-runtime bug: invalid code id"),
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
         .get(*index..*index + ENCODED_U16_SIZE)
         .expect("cab-runtime bug: invalid code id")
         .try_into()
         .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_U16_SIZE)
   }

   #[must_use]
   pub fn reserve_constant(&mut self, constant: Constant) -> ConstantIndex {
      let index = ConstantIndex(self.constants.len());
      self.constants.push(constant);
      index
   }

   #[must_use]
   pub fn read_constant(&self, index: ConstantIndex) -> &Constant {
      self
         .constants
         .get(*index)
         .expect("cab-runtime bug: invalid constant id")
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
      let position = self.spans.binary_search_by(|(id2, _)| id2.cmp(&index));

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

   /// Patches the operand of the jump at the given index to point to the *next*
   /// instruction will be emitted.
   pub fn patch_jump(&mut self, index: ByteIndex) {
      let offset = (self.content.len() - /* index: */ 1 - /* jump argument size: */ 2) as u16;

      self.content[*index + 1..*index + 2].copy_from_slice(&offset.to_le_bytes());
   }
}
