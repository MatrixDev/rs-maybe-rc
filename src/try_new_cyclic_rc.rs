use std::mem::MaybeUninit;
use std::rc::{Rc, Weak};

/// Helper function for creating cyclic `Rc` which might fail
///
/// # Example
///
/// ```rust
/// use std::rc::{Rc, Weak};
/// use maybe_rc::try_new_cyclic_rc;
///
/// struct Parent(Child);
/// struct Child(Weak<Parent>);
///
/// impl Parent {
///     fn try_new() -> Result<Rc<Self>, &'static str> {
///         try_new_cyclic_rc(|weak| {
///             let child = Child::try_new(weak.clone())?;
///             Ok(Self(child))
///         })
///     }
/// }
///
/// impl Child {
///     fn try_new(parent: Weak<Parent>) -> Result<Self, &'static str> {
///         let is_good_day = true;
///         match is_good_day {
///             true => Ok(Self(parent)),
///             false => Err("it is a bad day"),
///         }
///     }
/// }
/// ```
pub fn try_new_cyclic_rc<F, T, E>(f: F) -> Result<Rc<T>, E>
    where
        F: FnOnce(&Weak<T>) -> Result<T, E>,
{
    let mut error = None;

    let strong = Rc::<MaybeUninit<T>>::new_cyclic(|weak| {
        // SAFETY: T cannot be accessed from here, safe to strip down `MaybeUninit`
        let weak = unsafe {
            Weak::<T>::from_raw(weak.clone().into_raw().cast())
        };
        match f(&weak) {
            Err(e) => {
                error = Some(e);
                MaybeUninit::uninit()
            }
            Ok(e) => MaybeUninit::new(e),
        }
    });

    if let Some(error) = error {
        return Err(error);
    }

    // SAFETY: T is guaranteed to be initialized by now, safe to strip down `MaybeUninit`
    Ok(unsafe {
        Rc::from_raw(Rc::into_raw(strong).cast())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok() {
        struct Wrapper(usize, Weak<Wrapper>);

        let rc = try_new_cyclic_rc(|weak| {
            Ok::<Wrapper, ()>(Wrapper(42, weak.clone()))
        });

        assert!(rc.is_ok(), "must not fail");

        let rc = rc.unwrap();
        assert_eq!(rc.0, 42, "incorrect ok value");
        assert_eq!(rc.1.as_ptr(), Rc::as_ptr(&rc), "Weak and Rc point to a different objects");
    }

    #[test]
    fn test_err() {
        let rc = try_new_cyclic_rc(|_weak| {
            Err::<(), usize>(42)
        });

        assert!(rc.is_err(), "must fail");
        assert_eq!(rc, Err(42), "incorrect error value");
    }
}
