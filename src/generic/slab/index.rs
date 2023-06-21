use std::ptr::{null, null_mut};

/// A way to identify an item in a slab
pub trait Index: Copy + Eq {
    fn nowhere() -> Self;
    fn is_nowhere(&self) -> bool;
}

impl Index for usize {
    #[inline]
    fn nowhere() -> Self {
        usize::MAX
    }

    #[inline]
    fn is_nowhere(&self) -> bool {
        *self == usize::MAX
    }
}

impl<T> Index for *const T {
    #[inline]
    fn nowhere() -> Self {
        null()
    }

    #[inline]
    fn is_nowhere(&self) -> bool {
        self.is_null()
    }
}

impl<T> Index for *mut T {
    #[inline]
    fn nowhere() -> Self {
        null_mut()
    }

    #[inline]
    fn is_nowhere(&self) -> bool {
        self.is_null()
    }
}
