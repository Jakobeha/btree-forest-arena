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
