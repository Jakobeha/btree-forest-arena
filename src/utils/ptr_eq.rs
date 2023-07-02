use rustc_arena_modified::slab_arena::UnsafeRef;

pub trait PtrEq {
    /// Whether both pointers point to the same item
    fn ptr_eq(&self, other: &Self) -> bool;
}

impl<T> PtrEq for UnsafeRef<T> {
    #[inline]
    fn ptr_eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl PtrEq for u16 {
    #[inline]
    fn ptr_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl<T: PtrEq> PtrEq for Option<T> {
    #[inline]
    fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.ptr_eq(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<A: PtrEq, B: PtrEq> PtrEq for (A, B) {
    #[inline]
    fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0) && self.1.ptr_eq(&other.1)
    }
}