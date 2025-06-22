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

use cab_span::Span;
use derive_more::Deref;
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
      tag,
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

const ENCODED_U64_LEN: usize = 9;
const ENCODED_U16_LEN: usize = 0_u16.to_le_bytes().len();
const ENCODED_OPERATION_LEN: usize = 1;

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteIndex(usize);

impl ByteIndex {
   #[must_use]
   pub fn dummy() -> Self {
      Self(usize::MAX)
   }
}

#[derive(Deref, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
      let mut codes = VecDeque::from([(0_u64, self)]);

      while let Some((code_index, code)) = codes.pop_back() {
         let highlighted = RefCell::new(Vec::<ByteIndex>::new());
         let index_width = 2 + terminal::number_hex_width(code.bytes.len() - 1);

         let index = RefCell::new(ByteIndex(0));
         let mut index_previous = None::<usize>;
         terminal::indent!(writer, index_width + 3, |writer| {
            let index = *index.borrow();

            let style = if highlighted.borrow().contains(&index) {
               style::Style::new().cyan().bold()
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

               write!(writer, " {TOP_TO_BOTTOM} ")
            })?;

            index_previous.replace(index);
            Ok(index_width + 3)
         });

         if **index.borrow() < code.bytes.len() {
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
               write!(writer, "{code_index:#X}")
            })?;
         }

         let mut indent: usize = 0;

         while **index.borrow() < code.bytes.len() {
            let (_, operation, size) = code.read_operation(*index.borrow());

            terminal::indent!(
               writer,
               (indent - usize::from(operation == Operation::ScopeEnd)) * INDENT_WIDTH as usize,
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

            index.borrow_mut().0 += size;

            let mut arguments = operation.arguments().iter().enumerate().peekable();
            while let Some((argument_index, &argument)) = arguments.next() {
               if argument_index == 0 {
                  write(writer, &'('.bright_black().bold())?;
               }

               match argument {
                  Argument::U64 => {
                     let (u64, size) = code.read_u64(*index.borrow());

                     write(writer, &u64.blue())?;
                     index.borrow_mut().0 += size;
                  },

                  Argument::ValueIndex => {
                     let (value_index, size) = code.read_u64(*index.borrow());

                     let value_index_unique = code_index.add(2) * value_index.add(2);

                     with(writer, style::Color::Blue.fg().bold(), |writer| {
                        write!(writer, "{value_index:#X} ")
                     })?;
                     index.borrow_mut().0 += size;

                     match code[ValueIndex(
                        value_index
                           .try_into()
                           .expect("value index must fit in usize"),
                     )] {
                        Value::Blueprint(ref code) => {
                           codes.push_front((value_index_unique, code));
                           write(writer, &"-> ".bright_black().bold())?;
                           with(writer, style::Color::Red.fg().bold(), |writer| {
                              write!(writer, "{value_index_unique:#X}")
                           })?;
                        },

                        ref value => {
                           write(writer, &":: ".bright_black().bold())?;
                           Into::<tag::Tags<'_>>::into(value).display_styled(writer)?;
                        },
                     }
                  },

                  Argument::U16 => {
                     let (u16, size) = code.read_u16(*index.borrow());

                     write(writer, &u16.magenta())?;
                     index.borrow_mut().0 += size;
                  },

                  Argument::ByteIndex => {
                     let (u16, size) = code.read_u16(*index.borrow());

                     highlighted.borrow_mut().push(ByteIndex(u16 as _));

                     with(writer, style::Color::Cyan.fg().bold(), |writer| {
                        write!(writer, "{u16:#X}")
                     })?;
                     index.borrow_mut().0 += size;
                  },
               }

               write(
                  writer,
                  &if arguments.peek().is_none() { ')' } else { ',' }
                     .bright_black()
                     .bold(),
               )?;
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

impl Code {
   pub fn value(&mut self, value: Value) -> ValueIndex {
      let index = ValueIndex(self.values.len());
      self.values.push(value);
      index
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
      let encoded = self
         .bytes
         .get(*index..*index + ENCODED_U16_LEN)
         .expect("byte index must be valid")
         .try_into()
         .expect("size was statically checked");

      (u16::from_le_bytes(encoded), ENCODED_U16_LEN)
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
         (self.path.clone(), span),
         self.bytes[*index]
            .try_into()
            .expect("byte index must be valid"),
         ENCODED_OPERATION_LEN,
      )
   }

   pub fn point_here(&mut self, index: ByteIndex) {
      let here: u16 = self
         .bytes
         .len()
         .try_into()
         .expect("bytes len must fit in u16");

      self.bytes[*index..*index + ENCODED_U16_LEN].copy_from_slice(&here.to_le_bytes());
   }
}
