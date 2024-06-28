## Credits

- [`rnix-parser`](https://github.com/nix-community/rnix-parser): A lot was
  taken from the Nix parser implemented in Rust and rewritten. However I would
  not have figured out the best way to create a parser without the code
  in `rnix`. So thanks for that!

- [`rowan`](https://github.com/rust-analyzer/rowan): A great library for
  constructing syntax trees. The builder is especially relied upon by
  this library, and the parser would be much more challenging to implement
  without it.
