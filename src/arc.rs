use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::{Arc, Weak};

/// An uninitialized version of `Arc<T>`
///
/// This represents an `Arc<T>` that that doesn't contain any object inside
/// but still allows to construct a `Weak<T>` references.
///
/// Unlike `Arc<T>::new_cyclic` this object doesn't have the same constraints
/// and can be used in async function as well as for dependencies that might fail.
///
/// Since the new `MaybeArc<T>` is not fully-constructed until `MaybeArc<T>::materialize` is called,
/// calling upgrade on the weak reference will fail and result in a None value.
///
/// # Examples
///
/// ```
/// use std::sync::{Arc, Weak};
/// use maybe_rc::MaybeArc;
///
/// struct Parent {
///     child: Arc<Child>,
/// }
///
/// struct Child {
///     parent: Weak<Parent>,
/// }
///
/// impl Parent {
///     fn new() -> Result<Arc<Self>, String> {
///         let maybe = MaybeArc::new();
///         let child = Child::new(maybe.downgrade())?;
///         Ok(maybe.materialize(Self {
///             child,
///         }))
///     }
/// }
///
/// impl Child {
///     fn new(parent: Weak<Parent>) -> Result<Arc<Self>, String> {
///         Ok(Arc::new(Self { parent }))
///     }
/// }
/// ```
pub struct MaybeArc<T> {
    weak: Weak<UnsafeCell<MaybeUninit<T>>>,
}

impl<T> MaybeArc<T> {
    /// Constructs a new `MaybeArc<T>`.
    pub fn new() -> Self {
        let strong = Arc::new(UnsafeCell::new(MaybeUninit::uninit()));
        let weak = Arc::downgrade(&strong);
        Self { weak }
    }

    /// Creates a new `Weak<T>` pointer to this allocation.
    ///
    /// Upgrading this `Weak<T>` reference will fail and result in a None unless
    /// it is called after `MaybeArc<T>::materialize` finishes.
    pub fn downgrade(&self) -> Weak<T> {
        unsafe {
            std::mem::transmute(self.weak.clone())
        }
    }

    /// Materialize this allocation to a fully-contructed `Arc<T>`.
    ///
    /// All `Weak<T>` references can be upgraded after this method finishes.
    pub fn materialize(self, value: T) -> Arc<T> {
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
            Arc::increment_strong_count(ptr);
        }

        // SAFETY: we can transmute safely (unless std changes) weak into Arc because:
        // 1. their layout is the same
        // 2. strong ref count was just incremented
        // 3. weak counter must always be a at least 1 and we can guaranty that this
        //    will be the only Arc constructed for this allocation (look at Arc::Drop)
        unsafe {
            std::mem::transmute(self.weak)
        }
    }
}

impl<T> Default for MaybeArc<T> {
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
        let maybe = MaybeArc::<InnerT>::new();
        let arc = maybe.materialize(InnerT(&mut dropped));
        drop(arc);

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

        let maybe = MaybeArc::<InnerT>::new();
        drop(maybe);
    }

    #[test]
    fn test_weak_upgrade() {
        let maybe = MaybeArc::<usize>::new();

        let weak = maybe.downgrade();
        assert!(weak.upgrade().is_none(), "must not be upgradable");

        let arc = maybe.materialize(42);
        assert_eq!(weak.upgrade().map(|e| *e), Some(42), "must be upgradable");

        drop(arc);
        assert!(weak.upgrade().is_none(), "must not be upgradable");
    }
}
