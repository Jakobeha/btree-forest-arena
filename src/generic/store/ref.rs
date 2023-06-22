use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

#[cfg(any(doc, feature = "lock_api"))]
use lock_api::{MappedMutexGuard, MappedRwLockReadGuard, MappedRwLockWriteGuard, MutexGuard, RawMutex, RawRwLock, RwLockReadGuard, RwLockWriteGuard};

/// A shared reference, like `&'a T` or [`std::cell::Ref`]`<'a, T>`.
pub trait Ref<'a, T: ?Sized>: Deref<Target=T> {
    type Mapped<U: ?Sized + 'a>: Ref<'a, U>;

    /// Return a new reference to part of the referenced value
    fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U>;
    /// Return a new reference to part of the referenced value, or `Err` with `self` if the function
    /// returns `None`.
    ///
    /// The function MUST return `Ok` iff `f` returns `Some`.
    fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&T) -> Option<&U>
    ) -> Result<Self::Mapped<U>, Self> where Self: Sized;
    /// This law holds but can't be enforced by the type system, so we have enforce and enable using
    /// it by defining this conversion method:
    ///
    /// ```text
    /// for<R: Ref<T>, T, U, V> R::Mapped<U>::Mapped<V> = R::Mapped<V>
    /// ```
    ///
    /// ```text
    /// #[inline]
    /// fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
    ///     r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    /// ) -> Self::Mapped<V> {
    ///     r#ref
    /// }
    /// ```
    ///
    /// Note that `for<R: Ref<T>, T, U> R<T>::Mapped<U> = R<U>` *doesn't* always hold, e.g. mutex
    /// guards like those from [lock_api](https://docs.rs/lock_api) have different "mapped" types
    /// since the mapped types have less capabilities.
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V>;

    /// Gets the pointer to the underlying value
    fn as_ptr(&self) -> *const T {
        self.deref() as *const T
    }

    //noinspection DuplicatedCode
    /// Return a new reference to part of the referenced value, or `Err` with `self` and the error
    /// if the function returns `None`.
    #[inline]
    fn try_map2<U, E>(
        self,
        f: impl FnOnce(&T) -> Result<&U, E>
    ) -> Result<Self::Mapped<U>, (E, Self)> where Self: Sized {
        let mut error = MaybeUninit::uninit();
        self.try_map(|this| {
            match f(this) {
                Ok(result) => Some(result),
                Err(e) => {
                    error.write(e);
                    None
                }
            }
        }).map_err(|this| {
            // SAFETY: error is set when try_map fails, which is when we reach this Err case
            (unsafe { error.assume_init() }, this)
        })
    }

    //noinspection DuplicatedCode
    /// Return a new reference to part of the referenced value, or `Err` with `self` and the error
    /// if the function returns `None`.
    #[inline]
    fn try_map3<U, E>(
        self,
        f: impl FnOnce(&T) -> Result<Option<&U>, E>
    ) -> Result<Self::Mapped<U>, (Option<E>, Self)> where Self: Sized {
        let mut error = None;
        self.try_map(|this| {
            match f(this) {
                Ok(result) => result,
                Err(e) => {
                    error = Some(e);
                    None
                }
            }
        }).map_err(|this| (error, this))
    }
}

/// A mutable reference, like `&'a mut T` or [`std::cell::RefMut`]`<'a, T>`.
pub trait RefMut<'a, T: ?Sized>: DerefMut<Target=T> {
    type Mapped<U: ?Sized + 'a>: RefMut<'a, U>;

    /// Return a new reference to part of the referenced value
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U>;
    /// Return a new reference to part of the referenced value, or `Err` with `self` if the function
    /// returns `None`.
    fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>
    ) -> Result<Self::Mapped<U>, Self> where Self: Sized;
    /// Gets the mutable pointer to the underlying value
    fn as_mut_ptr(&mut self) -> *mut T {
        self.deref_mut() as *mut T
    }
    /// This law holds but can't be enforced by the type system, so we have enforce and enable using
    /// it by defining this conversion method:
    ///
    /// ```text
    /// for<R: RefMut<T>, T, U, V> R::Mapped<U>::Mapped<V> = R::Mapped<V>
    /// ```
    ///
    /// ```text
    /// #[inline]
    /// fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
    ///     r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    /// ) -> Self::Mapped<V> {
    ///     r#ref
    /// }
    /// ```
    ///
    /// Note that `for<R: RefMut<T>, T, U> R<T>::Mapped<U> = R<U>` *doesn't* always hold, e.g. mutex
    /// guards like those from [lock_api](https://docs.rs/lock_api) have different "mapped" types
    /// since the mapped types have less capabilities.
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V>;

    //noinspection DuplicatedCode
    /// Return a new reference to part of the referenced value, or `Err` with `self` and the error
    /// if the function returns `None`.
    #[inline]
    fn try_map2<U, E>(
        self,
        f: impl FnOnce(&mut T) -> Result<&mut U, E>
    ) -> Result<Self::Mapped<U>, (E, Self)> where Self: Sized {
        let mut error = MaybeUninit::uninit();
        self.try_map(|this| {
            match f(this) {
                Ok(result) => Some(result),
                Err(e) => {
                    error.write(e);
                    None
                }
            }
        }).map_err(|this| {
            // SAFETY: error is set when try_map fails, which is when we reach this Err case
            (unsafe { error.assume_init() }, this)
        })
    }

    //noinspection DuplicatedCode
    /// Return a new reference to part of the referenced value, or `Err` with `self` and the error
    /// if the function returns `None`.
    #[inline]
    fn try_map3<U, E>(
        self,
        f: impl FnOnce(&mut T) -> Result<Option<&mut U>, E>
    ) -> Result<Self::Mapped<U>, (Option<E>, Self)> where Self: Sized {
        let mut error = None;
        self.try_map(|this| {
            match f(this) {
                Ok(result) => result,
                Err(e) => {
                    error = Some(e);
                    None
                }
            }
        }).map_err(|this| (error, this))
    }
}

impl<'a, T: ?Sized> Ref<'a, T> for &'a T {
    type Mapped<U: ?Sized + 'a> = &'a U;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        f(self)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> Option<&U>) -> Result<Self::Mapped<U>, Self> {
        f(self).ok_or(self)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

impl<'a, T: ?Sized> RefMut<'a, T> for &'a mut T {
    type Mapped<U: ?Sized + 'a> = &'a mut U;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        f(self)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        let this = self as *mut T;
        match f(self) {
            // SAFETY: In the `None` case, `self` is not actually borrowed
            None => Err(unsafe { &mut *this }),
            Some(result) => Ok(result),
        }
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

impl<'a, T: ?Sized> Ref<'a, T> for std::cell::Ref<'a, T> {
    type Mapped<U: ?Sized + 'a> = std::cell::Ref<'a, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        std::cell::Ref::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> Option<&U>) -> Result<Self::Mapped<U>, Self> {
        std::cell::Ref::filter_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

impl<'a, T: ?Sized> RefMut<'a, T> for std::cell::RefMut<'a, T> {
    type Mapped<U: ?Sized + 'a> = std::cell::RefMut<'a, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        std::cell::RefMut::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        std::cell::RefMut::filter_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawRwLock, T: ?Sized> Ref<'a, T> for RwLockReadGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedRwLockReadGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        RwLockReadGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> Option<&U>) -> Result<Self::Mapped<U>, Self> {
        RwLockReadGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawRwLock, T: ?Sized> Ref<'a, T> for MappedRwLockReadGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedRwLockReadGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        MappedRwLockReadGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> Option<&U>) -> Result<Self::Mapped<U>, Self> {
        MappedRwLockReadGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawRwLock, T: ?Sized> RefMut<'a, T> for RwLockWriteGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedRwLockWriteGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        RwLockWriteGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        RwLockWriteGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawRwLock, T: ?Sized> RefMut<'a, T> for MappedRwLockWriteGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedRwLockWriteGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        MappedRwLockWriteGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        MappedRwLockWriteGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawMutex, T: ?Sized> RefMut<'a, T> for MutexGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedMutexGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        MutexGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        MutexGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

#[cfg(any(doc, feature = "lock_api"))]
impl<'a, R: RawMutex, T: ?Sized> RefMut<'a, T> for MappedMutexGuard<'a, R, T> {
    type Mapped<U: ?Sized + 'a> = MappedMutexGuard<'a, R, U>;

    #[inline]
    fn map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        MappedMutexGuard::map(self, f)
    }

    #[inline]
    fn try_map<U: ?Sized>(self, f: impl FnOnce(&mut T) -> Option<&mut U>) -> Result<Self::Mapped<U>, Self> {
        MappedMutexGuard::try_map(self, f)
    }

    //noinspection DuplicatedCode
    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}
