use std::{cell::Cell, error::Error, fmt, marker::PhantomData, ptr::NonNull, rc::Rc};

struct LeaseInner<T> {
    ptr: NonNull<T>,
    valid: Rc<Cell<bool>>,
    _data: PhantomData<T>,
}

#[derive(Debug, Clone, Copy)]
pub struct LeaseRevoked;

impl fmt::Display for LeaseRevoked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Lease was accessed after being revoked")
    }
}
impl Error for LeaseRevoked {}

pub struct LeaseRef<T> {
    inner: LeaseInner<T>,
}

impl<T> LeaseRef<T> {
    pub fn with<R>(&self, func: impl FnOnce(&T) -> R) -> Result<R, LeaseRevoked> {
        if self.inner.valid.get() {
            // See LeaseMut::with for safety
            Ok(func(unsafe { self.inner.ptr.as_ref() }))
        } else {
            Err(LeaseRevoked)
        }
    }
}

impl<T> Clone for LeaseRef<T> {
    fn clone(&self) -> Self {
        Self {
            inner: LeaseInner {
                ptr: self.inner.ptr,
                valid: self.inner.valid.clone(),
                _data: PhantomData,
            },
        }
    }
}

pub struct LeaseMut<T> {
    inner: LeaseInner<T>,
}

impl<T> LeaseMut<T> {
    pub fn with<R>(&mut self, func: impl FnOnce(&mut T) -> R) -> Result<R, LeaseRevoked> {
        if self.inner.valid.get() {
            // Safety:
            // The pointer validity has been checked
            // and it can't be smuggled out of func.
            // Since the Lease is !Send, no concurrent
            // access to the valid flag is possible
            // and there can't be any reference when
            // `lease(args, func)` returns.
            Ok(func(unsafe { self.inner.ptr.as_mut() }))
        } else {
            Err(LeaseRevoked)
        }
    }
}

pub struct LeaseToken {
    inner: Rc<Cell<bool>>,
}

pub trait Lease {
    type Output;

    fn make_lease(self, token: &LeaseToken) -> Self::Output;
}

impl<T> Lease for &T {
    type Output = LeaseRef<T>;

    fn make_lease(self, token: &LeaseToken) -> Self::Output {
        let inner = LeaseInner {
            ptr: self.into(),
            valid: token.inner.clone(),
            _data: PhantomData,
        };
        LeaseRef { inner }
    }
}

impl<T> Lease for &mut T {
    type Output = LeaseMut<T>;

    fn make_lease(self, token: &LeaseToken) -> Self::Output {
        let inner = LeaseInner {
            ptr: self.into(),
            valid: token.inner.clone(),
            _data: PhantomData,
        };
        LeaseMut { inner }
    }
}

macro_rules! impl_tuple {
    ($($name:ident),*$(,)?) => {
        impl<$($name,)*> Lease for ($($name,)*)
        where
            $($name: Lease,)*
        {
            type Output = ($($name::Output,)*);

            fn make_lease(self, token: &LeaseToken) -> Self::Output {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                ($($name.make_lease(token),)*)
            }
        }
    };
}

impl_tuple!(T1);
impl_tuple!(T1, T2);
impl_tuple!(T1, T2, T3);
impl_tuple!(T1, T2, T3, T4);
impl_tuple!(T1, T2, T3, T4, T5);
impl_tuple!(T1, T2, T3, T4, T5, T6);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
impl_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

pub fn lease<T: Lease, R>(args: T, func: impl FnOnce(T::Output) -> R) -> R {
    let valid = Rc::new(Cell::new(true));
    let token = LeaseToken {
        inner: valid.clone(),
    };
    let ret = func(args.make_lease(&token));
    valid.set(false);
    ret
}
