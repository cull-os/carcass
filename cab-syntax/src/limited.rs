#[derive(Debug, Clone)]
pub(crate) struct Limited<I: Iterator> {
    limit: usize,

    iterator: I,
    peeked: Option<Option<I::Item>>,
}

impl<I: Iterator> Limited<I> {
    pub fn new(iterator: I) -> Self {
        Self {
            limit: usize::MAX,

            iterator,
            peeked: None,
        }
    }

    pub fn peek(&mut self) -> Option<&I::Item> {
        let iterator = &mut self.iterator;
        self.peeked.get_or_insert_with(|| iterator.next()).as_ref()
    }

    pub fn next(&mut self) -> Option<I::Item> {
        if self.limit == 0 {
            None
        } else {
            self.limit -= 1;

            match self.peeked.take() {
                Some(next) => next,
                None => self.iterator.next(),
            }
        }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }
}
