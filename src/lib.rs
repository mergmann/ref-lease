use std::{cell::Cell, error::Error, fmt, marker::PhantomData, ptr::NonNull, rc::Rc};

/// This error is returned when trying to access a lease after it has been revoked
#[derive(Debug, Clone, Copy)]
pub struct LeaseRevoked;

impl fmt::Display for LeaseRevoked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Lease was accessed after being revoked")
    }
}
impl Error for LeaseRevoked {}

/// Lease for an `&T`
pub struct LeaseRef<T> {
    ptr: NonNull<T>,
    valid: Rc<Cell<bool>>,
    _data: PhantomData<*const T>,
}

impl<T> LeaseRef<T> {
    pub fn with<R>(&self, func: impl FnOnce(&T) -> R) -> Result<R, LeaseRevoked> {
        if self.valid.get() {
            // See LeaseMut::with for safety
            Ok(func(unsafe { self.ptr.as_ref() }))
        } else {
            Err(LeaseRevoked)
        }
    }
}

impl<T> Clone for LeaseRef<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            valid: self.valid.clone(),
            _data: PhantomData,
        }
    }
}

/// Lease for an `&mut T`
pub struct LeaseMut<T> {
    ptr: NonNull<T>,
    valid: Rc<Cell<bool>>,
    _data: PhantomData<*mut T>,
}

impl<T> LeaseMut<T> {
    pub fn with<R>(&mut self, func: impl FnOnce(&mut T) -> R) -> Result<R, LeaseRevoked> {
        if self.valid.get() {
            // Safety:
            // The pointer validity has been checked
            // and it can't be smuggled out of func.
            // Since the Lease is !Send, no concurrent
            // access to the valid flag is possible
            // and there can't be any reference when
            // `lease(args, func)` returns.
            Ok(func(unsafe { self.ptr.as_mut() }))
        } else {
            Err(LeaseRevoked)
        }
    }
}

pub struct LeaseToken(Rc<Cell<bool>>);

/// The trait for making something leasable
/// Implement this if you have a complex struct.
/// Example:
/// ```
/// use ref_lease::{Lease, LeaseMut, LeaseRef, LeaseToken, lease};
///
/// struct MyStruct<'a> {
///     field1: &'a u32,
///     field2: &'a mut String,
/// }
///
/// struct MyStructLease {
///     field1: LeaseRef<u32>,
///     field2: LeaseMut<String>,
/// }
///
/// unsafe impl<'a> Lease for MyStruct<'a> {
///     type Output = MyStructLease;
///
///     fn make_lease(self, token: &LeaseToken) -> Self::Output {
///         MyStructLease {
///             // Safety:
///             // We're only using fields on self, no temporary borrows
///             field1: self.field1.make_lease(token),
///             field2: self.field2.make_lease(token),
///         }
///     }
/// }
///
/// let value1 = 4;
/// let mut value2 = "hello".to_owned();
/// let value = MyStruct {
///     field1: &value1,
///     field2: &mut value2,
/// };
///
/// // Lease the entire struct at once
/// let v1 = lease(value, |mut lease| {
///     // access its fields
///     lease
///         .field2
///         .with(|value| value.push_str(" world!"))
///         // This does not fail because we're using it inside of the callback
///         .unwrap();
///     lease.field1.with(|value| *value).unwrap()
/// });
///
/// assert_eq!(value1, v1);
/// assert_eq!(value2, "hello world!");
/// ```
///
/// # Safety:
/// When implementing `Lease`, make sure all references
/// outlive `self`. No temporary borrows or references
/// to owned fields of `self` are allowed.
/// Additionally, keep the mutable borrow invariance:
/// There should only ever be one `LeaseMut` to the
/// same memory and there must be no `LeaseRef`s to it.
pub unsafe trait Lease {
    type Output;

    fn make_lease(self, token: &LeaseToken) -> Self::Output;
}

unsafe impl<T> Lease for &T {
    type Output = LeaseRef<T>;

    fn make_lease(self, token: &LeaseToken) -> Self::Output {
        LeaseRef {
            ptr: self.into(),
            valid: token.0.clone(),
            _data: PhantomData,
        }
    }
}

unsafe impl<T> Lease for &mut T {
    type Output = LeaseMut<T>;

    fn make_lease(self, token: &LeaseToken) -> Self::Output {
        LeaseMut {
            ptr: self.into(),
            valid: token.0.clone(),
            _data: PhantomData,
        }
    }
}

macro_rules! impl_tuple {
    ($($name:ident),*$(,)?) => {
        unsafe impl<$($name,)*> Lease for ($($name,)*)
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

/// Lease a reference by checking its lifetime at runtime.
/// This allows reference to be used freely while inside `func`,
/// without being bound by `reference`s lifetime.
/// Returning from func invalidates all leases to `reference`,
/// `lease.with` will then yield `LeaseRevoked`.
///
/// Example:
/// ```
/// use ref_lease::lease;
///
/// let mut hello = "hello".to_owned();
/// let mut hello_lease = lease(&mut hello, |mut lease| {
///     // 'lease' can now be given to python or any other FFI.
///     // As long as we're inside the callback to lease(), access is fine.
///     let result = lease.with(|hello| hello.push_str(" world!"));
///     assert!(result.is_ok());
///     lease
/// });
///
/// // Outside of the callback, the lease is invalid and 'with' will fail.
/// let result = hello_lease.with(|hello| hello.push_str("nope"));
/// assert!(result.is_err())
/// ```
pub fn lease<T: Lease, R>(reference: T, func: impl FnOnce(T::Output) -> R) -> R {
    // Invalidate lease on panic
    struct RAIIGuard(Rc<Cell<bool>>);
    impl Drop for RAIIGuard {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }

    let valid = Rc::new(Cell::new(true));
    let token = LeaseToken(valid.clone());
    let _guard = RAIIGuard(valid);
    func(reference.make_lease(&token))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tuple() {
        use super::lease;

        let (v1, mut v2, v3) = (2, 4, 6);
        let tuple = (&v1, &mut v2, &v3);

        lease(tuple, |(_l1, _l2, _l3)| {});
    }
}
