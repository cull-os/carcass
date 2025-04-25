# Islands

```cab
<self:{ revision = "xxxxx" }:/sub/path>/non/locked/path
# ^^^ ^^^^^^^^ configuration  ^ subpath ^ outer path
# format
```

The format is the underlying island implementation here; `self` is the "current
git repository", or whatever. Can be git, jj, fossil, whatever.

The subpath is the path that we nagivate to _before_ locking. The hash/lock
depends on the contents in that path.

Once we get the island and force any path within it, we hash it all. Outer path
accesses do not influence the island lock.

We can also omit the configuration and inner path, like so:

```cab
<self>
```

Provide only a configuration:

```cab
<self:{ revision = "xxxxx" }>
```

Provide only a subpath:

```cab
<self::/sub/path>
```

`self` doesn't have any locking, as it is the current git repository anyway. So
that expression above is pointless.

But this is not:

```cab
<fs::/home/foo>
```

That will only lock the `foo` home directory. While this (bad) would lock the
whole of `/home`:

```cab
<fs::/home>/foo
```

Parsing this syntax is straightforward, you parse a stringlike that starts with
`<` and ends with `:` or `>` for the format, if the ending was `:`, parse an
expression (perhaps `Noder::node_expression_single`, or a binding power one that
doesn't consume the cons operator), if the next is a `:` parse the `:` and
another expression for a subpath, and then parse the `>`.

After parsing, feed all these (format (content): `Option<Thunk>`, config:
`Option<Thunk>` and subpath: `Option<Thunk>`) into an Island thunk, and when
forced, force the format, pick the island implementation off of that (perhaps
this could be in `cab-island`, as a perfect hashmap for a `&str`) and have it
error depending on the arguments & if not return us our Island.

## Locking

This is tricky. We might want to lock depending on where the island literal was
declared (as in files), but then we would lose the flexibility of having lambdas
that return islands by formatting. Maybe we want that tradeoff? Or we could make
the user explicitly pin islands in the place they want, but that requires more
care and will make the language clunky. I believe we should pick the "lock to
source.lock.cab" approach where source is from the literal.

For the entrypoint which is stdin, we will make an exception and make it not
lock anything at all. Maybe we should generalize this to all leafs which don't
have parents? But that is a bad idea, you can't get a sister of
`<fs::/path/to/foo.cab>` as there is no parent, which might be unexpected
behaviour. Maybe we should store it inline? Like in the file itself, modify the
island directly in source code for locking.

That is a perfect idea and every island will be required to output itself with a
fully locked (ie. non-loose) version. Then we can format it and replace it in
the source code.

For this I will need to work on:

1. An evaluator.
2. A formatter.
3. The island stuff, finally.

Let's get to work (and read a few STG papers & the Snix codebase).
