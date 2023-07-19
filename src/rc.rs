use std::mem::MaybeUninit;
use std::rc::{Rc, Weak};

/// An uninitialized version of `Rc<T>`
///
/// This represents an `Rc<T>` that that doesn't contain any object inside
/// but still allows to construct a `Weak<T>` references.
///
/// Unlike `Rc<T>::new_cyclic` this object doesn't have the same constraints
/// and can be used in async function as well as for dependencies that might fail.
///
/// Since the new `MaybeRc<T>` is not fully-constructed until `MaybeRc<T>::materialize` is called,
/// calling upgrade on the weak reference will fail and result in a None value.
///
/// # Examples
///
/// ```
/// use std::rc::{Rc, Weak};
/// use maybe_rc::MaybeRc;
///
/// struct Parent {
///     child: Rc<Child>,
/// }
///
/// struct Child {
///     parent: Weak<Parent>,
/// }
///
/// impl Parent {
///     fn new() -> Result<Rc<Self>, String> {
///         let maybe = MaybeRc::new();
///         let child = Child::new(maybe.downgrade())?;
///         Ok(maybe.materialize(Self {
///             child,
///         }))
///     }
/// }
///
/// impl Child {
///     fn new(parent: Weak<Parent>) -> Result<Rc<Self>, String> {
///         Ok(Rc::new(Self { parent }))
///     }
/// }
/// ```
pub struct MaybeRc<T> {
    weak: Weak<T>,
}

impl<T> MaybeRc<T> {
    /// Constructs a new `MaybeRc<T>`.
    pub fn new() -> Self {
        let strong = Rc::new(MaybeUninit::<T>::uninit());

        // SAFETY: `MaybeUninit` is [repr(transparent)] so it can
        // be `stripped` down as memory layout should be the same
        let weak = unsafe {
            Weak::from_raw(Rc::downgrade(&strong).into_raw().cast())
        };

        Self { weak }
    }

    /// Creates a new `Weak<T>` pointer to this allocation.
    ///
    /// Upgrading this `Weak<T>` reference will fail and result in a None unless
    /// it is called after `MaybeRc<T>::materialize` finishes.
    pub fn downgrade(&self) -> Weak<T> {
        self.weak.clone()
    }

    /// Materialize this allocation to a fully-contructed `Rc<T>`.
    ///
    /// All `Weak<T>` references can be upgraded after this method finishes.
    pub fn materialize(self, value: T) -> Rc<T> {
        let ptr = self.weak.into_raw();

        // SAFETY: this value was not initialized before so
        // we need to update it without dropping the old value
        unsafe {
            std::ptr::write(ptr.cast_mut(), value);
        }

        // SAFETY: we hold a weak reference so content is still allocated
        // ASSUMPTION: we can restore `Rc` from strong count of 0
        unsafe {
            // increment strong count to 1, so weak can be upgraded
            Rc::increment_strong_count(ptr);
        }

        // SAFETY: `UnsafeCell` with `MaybeUninit` are `#[repr(transparent)]` so they
        // can be `stripped` down as memory layout should be the same
        unsafe {
            // we can consume Weak and make Rc from it because
            // at this point strong = 1 and weak = 1
            Rc::from_raw(ptr.cast())
        }
    }
}

impl<T> Default for MaybeRc<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_value() {
        let maybe = MaybeRc::<usize>::new();
        let rc = maybe.materialize(42);

        assert_eq!(*rc, 42, "value is not what was provided");
    }

    #[test]
    fn test_drop_init() {
        struct InnerT<'a>(&'a mut bool);

        impl<'a> Drop for InnerT<'a> {
            fn drop(&mut self) {
                *self.0 = true;
            }
        }

        let mut dropped = false;
        let maybe = MaybeRc::<InnerT>::new();
        let rc = maybe.materialize(InnerT(&mut dropped));
        drop(rc);

        assert!(dropped, "must be dropped");
    }

    #[test]
    fn test_drop_uninit() {
        struct InnerT;

        impl Drop for InnerT {
            fn drop(&mut self) {
                panic!("must not be dropped");
            }
        }

        let maybe = MaybeRc::<InnerT>::new();
        drop(maybe);
    }

    #[test]
    fn test_weak_upgrade() {
        let maybe = MaybeRc::<usize>::new();

        let weak = maybe.downgrade();
        assert!(weak.upgrade().is_none(), "must not be upgradable");

        let rc = maybe.materialize(42);
        assert_eq!(weak.upgrade().map(|e| *e), Some(42), "must be upgradable");

        drop(rc);
        assert!(weak.upgrade().is_none(), "must not be upgradable");
    }
}
