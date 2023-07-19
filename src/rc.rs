use std::cell::UnsafeCell;
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
    weak: Weak<UnsafeCell<MaybeUninit<T>>>,
}

impl<T> MaybeRc<T> {
    /// Constructs a new `MaybeRc<T>`.
    pub fn new() -> Self {
        // allocate Rc (strong = 1, weak = 1)
        let strong = Rc::new(UnsafeCell::new(MaybeUninit::uninit()));
        // create Weak (strong = 1, weak = 2)
        Self { weak: Rc::downgrade(&strong) }
        // drop Rc (strong = 0, weak = 1)
    }

    /// Creates a new `Weak<T>` pointer to this allocation.
    ///
    /// Upgrading this `Weak<T>` reference will fail and result in a None unless
    /// it is called after `MaybeRc<T>::materialize` finishes.
    pub fn downgrade(&self) -> Weak<T> {
        unsafe {
            std::mem::transmute(self.weak.clone())
        }
    }

    /// Materialize this allocation to a fully-contructed `Rc<T>`.
    ///
    /// All `Weak<T>` references can be upgraded after this method finishes.
    pub fn materialize(self, value: T) -> Rc<T> {
        let ptr = self.weak.as_ptr();

        // SAFETY: we know that memory is still allocated because of the weak
        // reference and no one can have access to it without unsafe code because
        // weak is non-upgradable at this point
        unsafe {
            let maybe_uninit = (*ptr).get();
            let maybe_uninit = &mut *maybe_uninit;
            maybe_uninit.write(value);
        }

        // SAFETY: we hold a weak reference so content is still allocated
        // ASSUMPTION: we can restore `Rc` from strong count of 1
        unsafe {
            // increment strong count to 1, so weak can be upgraded
            Rc::increment_strong_count(ptr);
        }

        // weak cannot fail here unless someone used unsafe from outside.
        // this will increment strong counter to 2
        let rc = self.weak.upgrade().unwrap();

        // forget weak so it doesn't decrement weak counter.
        // ASSUMPTION: unless std implementation changes all strong references
        // also collectively "hold" exactly 1 weak reference counter
        std::mem::forget(self.weak);

        // SAFETY: we hold a strong reference so content is allocated
        unsafe {
            // decrement strong counter back to 1 after upgrading weak reference
            Rc::decrement_strong_count(ptr);
        }

        // SAFETY: both UnsafeCell and MaybeUninit are repr(transparent) and
        // they can be safely stripped. MaybeUninit content was just initialized so we
        // can guarantee it is valid
        unsafe {
            std::mem::transmute(rc)
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
