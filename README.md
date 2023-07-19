# MaybeRc

This library provides a method of creating circular reference counted
dependencies when creation of the nested objects can fail (return Result).

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

But what to do in cases when `Child::new` returns an `Option` or `Result`?
This is the problem that maybe-rc tries to solve:
```rust
fn new() -> Result<Rc<Self>, ()> {
    try_new_cyclic_rc(|weak| {
        let child = Child::new(weak.clone())?;
        Ok(Self {
            child,
        })
    })
}
```

## Original Idea

Original idea was much more powerful and could also handle async functions but
was considered way too unsafe and breaking some contracts provided by `Rc`/`Arc`.
There is a hope that it will be possible to implement properly in the future as a library or even inside std.

Its description can be found at README.md.contested.