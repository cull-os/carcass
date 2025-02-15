mod size;
mod span;

pub use crate::{
    size::*,
    span::*,
};

// These two are kind of random but I'll include them here anyway.
// This crate is used everywhere.

#[macro_export]
macro_rules! into {
    ($($variable:ident),*) => {
        $(
            let $variable = $variable.into();
        )*
    }
}

pub fn paint<T>(value: T, style: yansi::Style) -> yansi::Painted<T> {
    use yansi::Paint as _;

    let mut painted = value.new();
    painted.style = style;
    painted
}
