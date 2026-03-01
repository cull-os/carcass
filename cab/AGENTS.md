- Use `jj`, never `git`.
- Prefer `cargo clippy` over `cargo check`.
- Prefer longer, more verbose identifiers, never super short anything unless it
  fits the existing code.
- Prefer not to import stuff, do `slotmap::new_key_type!` instead of `use`ing
  it.
- Do not spam `#[must_use]`, let `cargo clippy` tell you where to place it.
- Prefer direct macro metavariable names that mirror the underlying field or
  concept (for example `$field`).
- Do not over-specify identifiers. Do not do `left_expression` or
  `infix_operator` when `left` and `operator` suffice.
- Prefer to use `let ... else` when possible and appropriate.
