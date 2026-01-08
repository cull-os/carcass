# Carcass <!-- thank alcuin for the name -->

The Cull monorepository.

## Projects

- [`auxvec`](./auxvec/README.md): Auxiliary vector (auxv) reader and modifier.
- [`cab`](./cab/README.md): A dynamic, cacheable language (WIP).
- [`dup`](./dup/README.md): Cheap clone trait for distinguishing expensive
  clones from things that should have been Copy.
- [`ust`](./ust/mod.rs): Universal styling (WIP).

## Contributing

All contributors must follow the [Code of Conduct](./CODE_OF_CONDUCT.md).

## License

This project is subject to the terms of the Immutable Software License, edition
ee1e96f741ba9e18.

You can verify the notice's integrity using this POSIX shell script:

```sh
verify() {
  hash="$1";
  file="$2";

  [ $(sed "s/$hash/HASH-PLACEHOLDER/g" "$file" | sha256sum | head --bytes 16) = "$hash" ] && echo true || echo false;
}
```

Or this Nushell script:

```nu
def verify [ hash: string, file: path ]: nothing -> bool {
  open $file
  | str replace --all $hash "HASH-PLACEHOLDER"
  | hash sha256
  | str substring 0..<16
  | $in == $hash
}
```

And then by running `verify <edition-noted-above> LICENSE.md`
