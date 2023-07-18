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
///         let maybe_rc = MaybeRc::new();
///         let child = Child::new(maybe_rc.downgrade())?;
///         Ok(maybe_rc.materialize(Self {
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
        let strong = Rc::new(UnsafeCell::new(MaybeUninit::uninit()));
        let weak = Rc::downgrade(&strong);
        Self { weak }
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

        // SAFETY: memory is still held by the weak reference so we can increment
        // strong counter
        unsafe {
            Rc::increment_strong_count(ptr);
        }

        // SAFETY: we can transmute safely (unless std changes) weak into rc because:
        // 1. their layout is the same
        // 2. strong ref count was just incremented
        // 3. weak counter must always be a at least 1 and we can guaranty that this
        //    will be the only Rc constructed for this allocation (look at Rc::Drop)
        unsafe {
            std::mem::transmute(self.weak)
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
        let maybe_rc = MaybeRc::<InnerT>::new();
        let rc = maybe_rc.materialize(InnerT(&mut dropped));
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

        let maybe_rc = MaybeRc::<InnerT>::new();
        drop(maybe_rc);
    }

    #[test]
    fn test_weak_upgrade() {
        let maybe_rc = MaybeRc::<usize>::new();

        let weak = maybe_rc.downgrade();
        assert!(weak.upgrade().is_none(), "must not be upgradable");

        let rc = maybe_rc.materialize(42);
        assert_eq!(weak.upgrade().map(|e| *e), Some(42), "must be upgradable");

        drop(rc);
        assert!(weak.upgrade().is_none(), "must not be upgradable");
    }
}
