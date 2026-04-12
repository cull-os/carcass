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

## Error-handling

- `.expect()` messages must be full lowercase sentences explaining the invariant
  that makes the call infallible (e.g. `"size was statically checked"`,
  `"operator token must exist"`). No vague one-word messages like `"validated"`.
  Don't cite the caller or validation site (no
  `"(checked by Config::validate)"`). Every message must contain an invariant
  keyword: `must`, `cannot`, or `was`.
- `.expect()` is only for structurally infallible operations — the code itself
  must guarantee success (compile-time sizes, hardcoded literals, prior
  validation, self-references in data structures). Never for operations that
  depend on external or runtime state (channels, network, user input).
- Extract repeated `.expect()` messages to a `const EXPECT_*: &str` when the
  same message appears 3 or more times.
- `.unwrap()` is acceptable in tests and fuzz harnesses. In production code, use
  `.expect()` with a message instead.
