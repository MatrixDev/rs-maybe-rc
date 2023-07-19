# MaybeRc

This library provides a method of creating circular reference counted
dependencies when creation of the nested objects can fail (return Result)
or requires async function.

## Details

Usually `Rc::new_cyclic` can be used:
```rust
fn new() -> Rc<Self> {
    Rc::new_cyclic(|weak| {
        let child = Child::new(weak);
        Self {
            child,
        }
    })
}
```

But what to do in cases when `Child::new` is async or returns an `Option` or `Result`?
This is the problem that MaybeRc tries to solve:
```rust
async fn new() -> Option<Rc<Self>> {
    let maybe_rc = MaybeRc::<Self>::new();

    let weak = maybe_rc.downgrade();
    let child = Child::new(weak).await?;
    
    Some(maybe_rc.materialize(Self {
        child,
    }))
}
```

Same approach can be used for `Arc` types with `MaybeArc` implementation:
```rust
async fn new() -> Option<Arc<Self>> {
    let maybe_arc = MaybeArc::<Self>::new();

    let weak = maybe_arc.downgrade();
    let child = Child::new(weak).await?;

    Some(maybe_arc.materialize(Self {
        child,
    }))
}
```
## Unsafe Assumptions

Under the hood `MaybeRc` and `MaybeArc`  use some unsafe magic to implement this behavior.
Unfortunately, because standard library doesn't expose `Rc`/`Arc` internals, this magic
must rely on two assumptions:
1. `Rc<T>` and `Weak<T>` have the same content
2. All `Rc<T>` for the same allocation hold a single weak count until dropped 

Unless these assumptions are broken library will behave correctly.
