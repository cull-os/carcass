# auxvec

Auxiliary vector (auxv) reader and modifier.

## What is an "auxiliary vector"?

The auxiliary vector (auxv) is a sequence of key value pairs near the start of a
running ELF program's stack. **They do not exist within the ELF file, and are
instantiated by the kernel before the program is handed off to the interpreter
(or directly executed, if it does not specify one)**.

These key/value pairs give the interpreter (and dynamic linker, they are
different concepts but usually implemented by the same program) information on
what to do with the ELF file & how to transform its dynamic, relocatable code
into something that the CPU can execute.

The way this library actually find this vector is by following the address in
the global `environ` variable, which points to the start of the environment
variables (which is actually a C-style null terminated list of pointers to
C-style null terminated strings: `&CSlice<&CStr>`), and setting the start of the
vector to the element right after the environment null sentinel:

```text
++++++++++++++-----------------------------------------+
+ WORD SIZED * <pointer> -> "FOO=bar\0"                 |
++++++++++++++------------------------------------------+
+ WORD SIZED * <pointer> -> "BAR=baz\0"                 |
++++++++++++++------------------------------------------+
+ WORD SIZED * NULL (end of the environment)            |
++++++++++++++------------------------------------------+
+ WORD SIZED * <key>   (start of auxiliary vector)      |
++++++++++++++------------------------------------------+
+ WORD SIZED * <value> (the value for the first entry)  |
++++++++++++++-----------------------------------------+
```

And it is guaranteed that the vector ends with a `(key, value)` pair where the
`key` is 0. This is how we figure out to stop iterating.

You can see some of the common auxiliary vector keys set by the Linux kernel in
[this document](https://refspecs.linuxfoundation.org/ELF/zSeries/lzsabi0_zSeries/x895.html).

These keys are widely defined using macros in C and are prefixed using `AT_`.
However, this crate uses `auxvec::VectorKey::<type-here>` instead and doesn't
define the enumerations using the C naming style. `AT_PAGESZ` vs
`auxvec::VectorKey::PageSize`, for example.

TODO: write rest of the README and finish documenting `mod.rs`.

More reading on the auxiliary vector:

- Putting the "You" in CPU: <https://cpu.land/> (personal favourite, it's
  trivial to understand)
- (Manu Garg) About ELF Auxiliary Vectors:
  <http://articles.manugarg.com/aboutelfauxiliaryvectors.html>
- (Linux foundation refspec) Process initialization:
  <https://refspecs.linuxfoundation.org/ELF/zSeries/lzsabi0_zSeries/x895.html>
- (PHRACK) Armouring the ELF: Binary encryption on the UNIX platform:
  <https://phrack.org/issues/58/5>
- See the ELF binfmt in Linux to see how the vector is generated
  (`fs/binfmt_elf.c`).
- See `include/uapi/linux/auxvec.h` in the Linux source (or `man 3 getauxval`)
  for defined keys, as well as other header files for architecture-specific
  types.
- Searching for `AT_` in your OS of choice (it's open source, right?) will also
  yield some good results on how it's handled.

## Credits

- The well documented [`auxv` crate](https://lib.rs/auxv), which was a good
  starting point for the documentation.
- [`nix-ld`](https://github.com/nix-community/nix-ld), which inspired me to
  write a better and well-designed version of `auxv.rs`.
- And all the other people around the world that documented some of the odd
  keys.
