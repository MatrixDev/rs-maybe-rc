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
    let this = Self {
        child,
    };
    
    Some(maybe_rc.materialize(this))
}
```

## Assumptions

In order to provide this behaviour `MaybeRc` makes two assumptions:
1. `Rc<T>` and `Weak<T>` have the same content
2. All `Rc<T>` for the same allocation hold a single weak count until dropped 

Unless these assumptions are broken library will behave correctly.
