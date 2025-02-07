use crate::*;

#[derive(Debug)]
pub struct File<'a> {
    // TODO: Use a real island type.
    pub island: CowStr<'a>,
    pub path: CowStr<'a>,
    pub source: CowStr<'a>,
}
