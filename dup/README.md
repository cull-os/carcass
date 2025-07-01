# dup

Cheap clone trait for distinguishing expensive clones from things that should
have been Copy.

## Why?

It's easier to distinguish real clones from cheap copies when the method is
named `dupe` rather than `clone`.

Hopefully,
[The `Claim` trait](https://smallcultfollowing.com/babysteps/blog/2024/06/26/claim-followup-1/)
will remove the need for this entirely.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
dup = { path = "../path/to/dup", features = [ "see-note-below" ] }
```

You can enable additional features, such as `bytes` and `rpds` to get `Dupe`
implementations for other crates.

### By Itself

```rs
use dup::Dupe as _;

let arc = Arc::new(42);
let arc_ = arc.dupe();
```

### Custom Type

```rs
use dup::Dupe;

#[derive(Clone, Dupe)]
struct ArcHolder<T>(Arc<T>);

let arc_holder = ArcHolder(Arc::new(1453));
let arc_holder_ = arc_holder.dupe();
```
