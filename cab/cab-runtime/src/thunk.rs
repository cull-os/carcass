use std::ops;

use cab_why::Span;

use crate::{
    Operation,
    Value,
};

const ENCODED_U64_SIZE: usize = 9;
const ENCODED_U16_SIZE: usize = 2;

#[derive(Debug, Clone, Copy)]
pub struct CodeId(usize);

impl ops::Deref for CodeId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConstantId(usize);

impl ops::Deref for ConstantId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Thunk {
    code: Vec<u8>,
    spans: Vec<(CodeId, Span)>,

    constants: Vec<Value>,
}

impl Thunk {
    pub fn push_u64(&mut self, data: u64) -> CodeId {
        let mut encoded = [0; ENCODED_U64_SIZE];
        let len = vu128::encode_u64(&mut encoded, data);

        let id = CodeId(self.code.len());
        self.code.extend_from_slice(&encoded[..len]);
        id
    }

    pub fn read_u64(&self, id: CodeId) -> (u64, usize) {
        let encoded = match self.code.get(*id..*id + ENCODED_U64_SIZE) {
            Some(slice) => slice.try_into().expect("size statically checked"),

            None => {
                let mut buffer = [0; ENCODED_U64_SIZE];
                buffer[..self.code.len() - *id]
                    .copy_from_slice(self.code.get(*id..).expect("cab-runtime bug: invalid code id"));
                buffer
            },
        };

        vu128::decode_u64(&encoded)
    }

    pub fn push_u16(&mut self, data: u16) -> CodeId {
        let id = CodeId(self.code.len());
        self.code.extend_from_slice(&data.to_le_bytes());
        id
    }

    pub fn read_u16(&self, id: CodeId) -> (u16, usize) {
        let encoded = self
            .code
            .get(*id..*id + ENCODED_U16_SIZE)
            .expect("cab-runtime bug: invalid code id")
            .try_into()
            .expect("size statically checked");

        (u16::from_le_bytes(encoded), ENCODED_U16_SIZE)
    }

    pub fn push_constant(&mut self, value: Value) -> ConstantId {
        let id = self.constants.len();
        self.constants.push(value);
        ConstantId(id)
    }

    pub fn push_operation(&mut self, span: Span, operation: Operation) -> CodeId {
        let id = CodeId(self.code.len());
        self.code.push(operation as u8);

        // No need to insert the span again if this instruction was created from the
        // last span.
        if self.spans.last().is_none_or(|&(_, last_span)| last_span != span) {
            self.spans.push((id, span));
        }

        id
    }

    pub fn read_operation(&self, id: CodeId) -> (Span, Operation) {
        let position = self.spans.binary_search_by(|(id2, _)| id2.cmp(&id));

        let (id, span) = match position {
            Ok(index) => self.spans[index],
            Err(0) => self.spans[0],
            Err(index) => self.spans[index - 1],
        };

        (
            span,
            self.code[*id].try_into().expect("cab-runtime bug: invalid operation"),
        )
    }
}
