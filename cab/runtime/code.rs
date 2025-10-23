use std::{
   cell::RefCell,
   collections::VecDeque,
   fmt::{
      self,
      Write as _,
   },
   ops::{
      self,
      Add as _,
   },
};

use derive_more::{
   Deref,
   DerefMut,
};
use dup::Dupe as _;
use ranged::Span;
use ust::{
   COLORS,
   Display,
   INDENT_WIDTH,
   STYLE_GUTTER,
   Write,
   style::{
      self,
      StyledExt as _,
   },
   terminal::{
      self,
      DOT,
      LEFT_TO_RIGHT,
      RIGHT_TO_BOTTOM,
      TOP_TO_BOTTOM,
   },
   with,
   write,
};

use crate::{
   Argument,
   Location,
   Operation,
   Value,
   value,
};

const ENCODED_U64_LEN_MAX: usize = 9;
const ENCODED_U16_LEN_MAX: usize = 0_u16.to_le_bytes().len();
const ENCODED_OPERATION_LEN: usize = 1;

#[derive(Deref, DerefMut, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteIndex(usize);

impl ByteIndex {
   #[must_use]
   pub fn dummy() -> Self {
      Self(usize::MAX)
   }
}

#[derive(Deref, DerefMut, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValueIndex(usize);

impl ValueIndex {
   #[must_use]
   pub fn dummy() -> Self {
      Self(usize::MAX)
   }
}

pub struct Code {
   bytes: Vec<u8>,

   path:  value::Path,
   spans: Vec<(ByteIndex, Span)>,

   values: Vec<Value>,
}

impl Display for Code {
   fn display_styled(&self, writer: &mut dyn Write) -> fmt::Result {
      const STYLE_JUMP_ADDRESS: style::Style = style::Color::BrightYellow.fg().bold().underline();

      enum CodeType {
         Suspend,
         Lambda,
      }

      let mut codes = VecDeque::from([(0_usize, CodeType::Suspend, self)]);

      while let Some((code_index, code_type, code)) = codes.pop_back() {
         let highlighted = RefCell::new(Vec::<ByteIndex>::new());

         // INDENT: "0x123 | "
         let index_width = 2 + terminal::number_hex_width(code.bytes.len() - 1);
         let indent_index = RefCell::new(ByteIndex(0));
         let mut index_previous = None::<usize>;
         terminal::indent!(writer, index_width + 3, |writer| {
            let index = *indent_index.borrow();

            let style = if highlighted.borrow().contains(&index) {
               STYLE_JUMP_ADDRESS
            } else {
               STYLE_GUTTER
            };

            let index = *index;

            with(writer, style, |writer| {
               if index_previous == Some(index) {
                  let dot_width = 2 + terminal::number_hex_width(index);
                  let space_width = index_width - dot_width;

                  write!(writer, "{:>space_width$}", "")?;

                  for _ in 0..dot_width {
                     write!(writer, "{DOT}")?;
                  }
               } else {
                  write!(writer, "{index:>#index_width$X}")?;
               }

               write!(writer, " {TOP_TO_BOTTOM}")
            })?;

            index_previous.replace(index);
            Ok(index_width + 2)
         });

         if **indent_index.borrow() < code.bytes.len() {
            // DEDENT: "| "
            terminal::dedent!(writer, 2);

            // INDENT: "┏━━━ ".
            terminal::indent!(
               writer,
               header =
                  const_str::concat!(RIGHT_TO_BOTTOM, LEFT_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT)
                     .style(STYLE_GUTTER)
            );

            with(writer, style::Color::Red.fg().bold(), |writer| {
               write!(writer, "{code_index:#X} ")
            })?;

            match code_type {
               CodeType::Suspend => write(writer, &"(suspend)".cyan().bold())?,
               CodeType::Lambda => write(writer, &"(lambda)".magenta().bold())?,
            }
         }

         let mut indent: usize = 0;

         let mut items = code.iter().peekable();
         while let Some((index, item)) = items.next() {
            *indent_index.borrow_mut() = index;

            match item {
               CodeItem::Operation(operation) => {
                  terminal::indent!(
                     writer,
                     (indent - usize::from(operation == Operation::ScopeEnd))
                        * INDENT_WIDTH as usize,
                  );

                  writeln!(writer)?;

                  if operation == Operation::ScopeEnd {
                     indent -= 1;
                     write(writer, &"}".style(COLORS[indent % COLORS.len()]))?;
                     write!(writer, " ")?;
                  }

                  with(writer, style::Color::Yellow.fg(), |writer| {
                     write!(writer, "{operation:?}")
                  })?;

                  if operation == Operation::ScopeStart {
                     write!(writer, " ")?;
                     write(writer, &"{".style(COLORS[indent % COLORS.len()]))?;
                     indent += 1;
                  }

                  if let Some(&(_, CodeItem::Argument(_))) = items.peek() {
                     write(writer, &'('.bright_black().bold())?;
                  }
               },

               CodeItem::Argument(argument) => {
                  match argument {
                     Argument::U16(u16) => write(writer, &u16.magenta())?,

                     Argument::U64(u64) => write(writer, &u64.blue())?,

                     Argument::ValueIndex(value_index) => {
                        let value_index_unique = code_index.add(2) * value_index.add(2);

                        with(writer, style::Color::Blue.fg().bold(), |writer| {
                           write!(writer, "{value_index:#X} ", value_index = *value_index)
                        })?;

                        match code[value_index] {
                           ref value @ (Value::Suspend(ref code) | Value::Lambda(ref code)) => {
                              codes.push_front((
                                 value_index_unique,
                                 match *value {
                                    Value::Suspend(_) => CodeType::Suspend,
                                    Value::Lambda(_) => CodeType::Lambda,
                                    _ => unreachable!(),
                                 },
                                 code,
                              ));

                              write(writer, &"-> ".bright_black().bold())?;
                              with(writer, style::Color::Red.fg().bold(), |writer| {
                                 write!(writer, "{value_index_unique:#X}")
                              })?;
                           },

                           ref value => {
                              write(writer, &":: ".bright_black().bold())?;
                              value.display_styled(writer)?;
                           },
                        }
                     },

                     Argument::ByteIndex(byte_index) => {
                        highlighted.borrow_mut().push(byte_index);

                        with(writer, STYLE_JUMP_ADDRESS, |writer| {
                           write!(writer, "{byte_index:#X}", byte_index = *byte_index)
                        })?;
                     },
                  }

                  let delimiter = match items.peek() {
                     Some(&(_, CodeItem::Argument(_))) => ", ",
                     _ => ")",
                  };

                  write(writer, &delimiter.bright_black().bold())?;
               },
            }
         }

         if !codes.is_empty() {
            writeln!(writer)?;
            writeln!(writer)?;
         }
      }

      Ok(())
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeItem {
   Operation(Operation),
   Argument(Argument),
}

impl CodeItem {
   pub fn as_operation(&self) -> Option<&Operation> {
      if let &Self::Operation(ref operation) = self {
         Some(operation)
      } else {
         None
      }
   }

   pub fn as_argument(&self) -> Option<&Argument> {
      if let &Self::Argument(ref argument) = self {
         Some(argument)
      } else {
         None
      }
   }
}

impl Code {
   pub fn value(&mut self, value: Value) -> ValueIndex {
      let index = ValueIndex(self.values.len());
      self.values.push(value);
      index
   }

   pub fn iter(&self) -> impl Iterator<Item = (ByteIndex, CodeItem)> {
      gen move {
         let mut index = ByteIndex(0);

         while *index < self.bytes.len() {
            let (_, operation, size) = self.read_operation(index);

            yield (index, CodeItem::Operation(operation));
            *index += size;

            match operation {
               Operation::Push => {
                  let (value, size) = self.read_u64(index);

                  yield (
                     index,
                     CodeItem::Argument(Argument::ValueIndex(ValueIndex(
                        usize::try_from(value).expect("value index must be valid"),
                     ))),
                  );

                  *index += size;
               },

               Operation::Jump | Operation::JumpIf => {
                  let (value, size) = self.read_u16(index);

                  yield (
                     index,
                     CodeItem::Argument(Argument::ByteIndex(ByteIndex(usize::from(value)))),
                  );

                  *index += size;
               },

               Operation::Interpolate => {
                  let (value, size) = self.read_u64(index);

                  yield (index, CodeItem::Argument(Argument::U64(value)));

                  *index += size;
               },

               _ => {},
            }
         }
      }
   }
}

impl ops::Index<ValueIndex> for Code {
   type Output = Value;

   fn index(&self, index: ValueIndex) -> &Self::Output {
      self.values.get(*index).expect("value index must be valid")
   }
}

impl Code {
   #[must_use]
   pub fn new(path: value::Path) -> Self {
      Self {
         bytes: Vec::new(),

         path,
         spans: Vec::new(),

         values: Vec::new(),
      }
   }

   #[must_use]
   pub fn path(&self) -> &value::Path {
      &self.path
   }

   pub fn push_u64(&mut self, data: u64) -> ByteIndex {
      let mut encoded = [0; ENCODED_U64_LEN_MAX];
      let len = vu128::encode_u64(&mut encoded, data);

      let index = ByteIndex(self.bytes.len());
      self.bytes.extend_from_slice(&encoded[..len]);
      index
   }

   #[must_use]
   pub fn read_u64(&self, index: ByteIndex) -> (u64, usize) {
      let encoded = match self.bytes.get(*index..*index + ENCODED_U64_LEN_MAX) {
         Some(slice) => {
            <[u8; ENCODED_U64_LEN_MAX]>::try_from(slice).expect("size was statically checked")
         },

         None => {
            let mut buffer = [0; ENCODED_U64_LEN_MAX];
            buffer[..self.bytes.len() - *index]
               .copy_from_slice(self.bytes.get(*index..).expect("byte index must be valid"));
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
      let encoded = <[u8; ENCODED_U16_LEN_MAX]>::try_from(
         self
            .bytes
            .get(*index..*index + ENCODED_U16_LEN_MAX)
            .expect("byte index must be valid"),
      )
      .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_U16_LEN_MAX)
   }

   pub fn push_operation(&mut self, span: Span, operation: Operation) -> ByteIndex {
      let index = ByteIndex(self.bytes.len());
      self.bytes.push(operation as _);

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
   pub fn read_operation(&self, index: ByteIndex) -> (Location, Operation, usize) {
      let position = self.spans.partition_point(|&(index2, _)| index >= index2);

      let (_, span) = self.spans[position.saturating_sub(1)];

      (
         (self.path.dupe(), span),
         Operation::try_from(self.bytes[*index]).expect("byte index must be valid"),
         ENCODED_OPERATION_LEN,
      )
   }

   pub fn point_here(&mut self, index: ByteIndex) {
      let here = u16::try_from(self.bytes.len()).expect("bytes len must fit in u16");

      self.bytes[*index..*index + ENCODED_U16_LEN_MAX].copy_from_slice(&here.to_le_bytes());
   }
}
