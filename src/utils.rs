use std::mem::MaybeUninit;
pub use ptr_eq::*;

mod ptr_eq;

/// [MaybeUninit::uninit_array] but stable.
#[inline]
pub fn maybe_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
	unsafe { MaybeUninit::uninit().assume_init() }
}