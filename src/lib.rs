//! # beef
//!
//! Alternative implementation of `Cow` that's more compact in memory.
//!
//! ```rust
//! # fn main() {
//! use beef::Cow;
//!
//! let borrowed = Cow::borrowed("Hello");
//! let owned = Cow::from(String::from("World"));
//!
//! assert_eq!(
//!     format!("{} {}!", borrowed, owned),
//!     "Hello World!",
//! );
//!
//! // beef::Cow is 3 word sized, while std::borrow::Cow is 4 word sized
//! assert!(std::mem::size_of::<Cow<str>>() < std::mem::size_of::<std::borrow::Cow<str>>());
//! # }
//! ```

use std::borrow::{Borrow, ToOwned, Cow as StdCow};
use std::fmt;
use std::num::NonZeroUsize;
use std::hash::{Hash, Hasher};

#[derive(Eq)]
pub struct Cow<'a, T: Beef + ?Sized + 'a> {
    inner: &'a T,
    capacity: Option<NonZeroUsize>,
}

pub unsafe trait Beef: ToOwned {
    fn capacity(owned: &Self::Owned) -> Option<NonZeroUsize>;

    unsafe fn rebuild(&self, capacity: usize) -> Self::Owned;
}

unsafe impl Beef for str {
    #[inline]
    fn capacity(owned: &String) -> Option<NonZeroUsize> {
        NonZeroUsize::new(owned.capacity())
    }

    #[inline]
    unsafe fn rebuild(&self, capacity: usize) -> String {
        String::from_utf8_unchecked(
            Vec::from_raw_parts(self.as_ptr() as *mut u8, self.len(), capacity)
        )
    }
}

unsafe impl<T: Clone> Beef for [T] {
    #[inline]
    fn capacity(owned: &Vec<T>) -> Option<NonZeroUsize> {
        NonZeroUsize::new(owned.capacity())
    }

    #[inline]
    unsafe fn rebuild(&self, capacity: usize) -> Vec<T> {
        Vec::from_raw_parts(self.as_ptr() as *mut T, self.len(), capacity)
    }
}

impl<B> Cow<'_, B>
where
    B: Beef + ?Sized,
{
    #[inline]
    pub fn owned(val: B::Owned) -> Self {
        let inner = unsafe { &*(val.borrow() as *const B) };
        let capacity = B::capacity(&val);

        std::mem::forget(val);

        Cow {
            inner,
            capacity,
        }
    }
}

impl<'a, T> Cow<'a, T>
where
    T: Beef + ?Sized,
{
    // This can be made const fn in the future:
    // https://github.com/rust-lang/rust/issues/57563
    #[inline]
    pub fn borrowed(val: &'a T) -> Self {
        Cow {
            inner: val,
            capacity: None,
        }
    }

    #[inline]
    pub fn into_owned(self) -> T::Owned {
        let Cow { inner, capacity } = self;

        std::mem::forget(self);

        match capacity {
            Some(capacity) => unsafe {
                inner.rebuild(capacity.get())
            },
            None => inner.to_owned(),
        }
    }
}

impl<T> Hash for Cow<'_, T>
where
    T: Hash + Beef + ?Sized,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<'a, T> From<&'a T> for Cow<'a, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn from(val: &'a T) -> Self {
        Cow::borrowed(val)
    }
}

impl From<String> for Cow<'_, str> {
    #[inline]
    fn from(s: String) -> Self {
        Cow::owned(s)
    }
}

impl<T> From<Vec<T>> for Cow<'_, [T]>
where
    T: Clone,
{
    #[inline]
    fn from(v: Vec<T>) -> Self {
        Cow::owned(v)
    }
}

impl<T> Drop for Cow<'_, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn drop(&mut self) {
        if let Some(capacity) = self.capacity {
            std::mem::drop(unsafe {
                self.inner.rebuild(capacity.get())
            });
        }
    }
}

impl<'a, T> Clone for Cow<'a, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn clone(&self) -> Self {
        match self.capacity {
            Some(_) => Cow::owned(self.inner.to_owned()),
            None => Cow { ..*self }
        }
    }
}

impl<T> std::ops::Deref for Cow<'_, T>
where
    T: Beef + ?Sized,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsRef<T> for Cow<'_, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<T> Borrow<T> for Cow<'_, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn borrow(&self) -> &T {
        self.inner
    }
}

impl<'a, T> From<StdCow<'a, T>> for Cow<'a, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn from(stdcow: StdCow<'a, T>) -> Self {
        match stdcow {
            StdCow::Borrowed(v) => Self::borrowed(v),
            StdCow::Owned(v) => Self::owned(v),
        }
    }
}

impl<'a, T> From<Cow<'a, T>> for StdCow<'a, T>
where
    T: Beef + ?Sized,
{
    #[inline]
    fn from(cow: Cow<'a, T>) -> Self {
        let Cow { inner, capacity } = cow;

        std::mem::forget(cow);

        match capacity {
            Some(capacity) => StdCow::Owned(unsafe {
                inner.rebuild(capacity.get())
            }),
            None => StdCow::Borrowed(inner),
        }
    }
}


impl<T, U> PartialEq<U> for Cow<'_, T>
where
    T: Beef + PartialEq + ?Sized,
    U: AsRef<T> + ?Sized,
{
    #[inline]
    fn eq(&self, other: &U) -> bool {
        self.inner == other.as_ref()
    }
}

impl PartialEq<Cow<'_, str>> for str {
    #[inline]
    fn eq(&self, other: &Cow<str>) -> bool {
        self == other.inner
    }
}

impl PartialEq<Cow<'_, str>> for &str {
    #[inline]
    fn eq(&self, other: &Cow<str>) -> bool {
        *self == other.inner
    }
}

impl PartialEq<Cow<'_, str>> for String {
    #[inline]
    fn eq(&self, other: &Cow<str>) -> bool {
        self == other.inner
    }
}

impl<T> PartialEq<Cow<'_, [T]>> for [T]
where
    T: Clone + PartialEq,
    [T]: Beef,
{
    #[inline]
    fn eq(&self, other: &Cow<[T]>) -> bool {
        self == other.inner
    }
}

impl<T> PartialEq<Cow<'_, [T]>> for &[T]
where
    T: Clone + PartialEq,
    [T]: Beef,
{
    #[inline]
    fn eq(&self, other: &Cow<[T]>) -> bool {
        *self == other.inner
    }
}

impl<T> PartialEq<Cow<'_, [T]>> for Vec<T>
where
    T: Clone + PartialEq,
    [T]: Beef,
{
    #[inline]
    fn eq(&self, other: &Cow<[T]>) -> bool {
        &self[..] == other.inner
    }
}

impl<T: Beef + fmt::Debug + ?Sized> fmt::Debug for Cow<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: Beef + fmt::Display + ?Sized> fmt::Display for Cow<'_, T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::Cow;

    #[test]
    fn borrowed_str() {
        let s = "Hello World";
        let c = Cow::borrowed(s);

        assert_eq!(s, c);
        assert_eq!(s, c.as_ref());
        assert_eq!(s, &*c);
        assert_eq!(s, c.inner);
    }

    #[test]
    fn owned_string() {
        let s = String::from("Hello World");
        let c: Cow<str> = Cow::owned(s.clone());

        assert_eq!(s, c);
    }

    #[test]
    fn into_owned() {
        let hello = "Hello World";
        let borrowed = Cow::borrowed(hello);
        let owned: Cow<str> = Cow::owned(String::from(hello));

        assert_eq!(borrowed.into_owned(), hello);
        assert_eq!(owned.into_owned(), hello);
    }

    #[test]
    fn borrowed_slice() {
        let s: &[_] = &[1, 2, 42];
        let c = Cow::borrowed(s);

        assert_eq!(s, c);
        assert_eq!(s, c.as_ref());
        assert_eq!(s, &*c);
        assert_eq!(s, c.inner);
    }

    #[test]
    fn owned_slice() {
        let s = vec![1, 2, 42];
        let c: Cow<[_]> = Cow::owned(s.clone());

        assert_eq!(s, c);
    }

    #[test]
    fn into_owned_vec() {
        let hello: &[u8] = b"Hello World";
        let borrowed = Cow::borrowed(hello);
        let owned: Cow<[u8]> = Cow::owned(hello.to_vec());

        assert_eq!(borrowed.into_owned(), hello);
        assert_eq!(owned.into_owned(), hello);
    }

    #[test]
    fn hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let slice = "Hello World!";
        let borrowed = Cow::borrowed(slice);
        let owned: Cow<str> = Cow::owned(slice.to_owned());

        let hash1 = {
            let mut hasher = DefaultHasher::default();

            slice.hash(&mut hasher);

            hasher.finish()
        };

        let hash2 = {
            let mut hasher = DefaultHasher::default();

            borrowed.hash(&mut hasher);

            hasher.finish()
        };

        let hash3 = {
            let mut hasher = DefaultHasher::default();

            owned.hash(&mut hasher);

            hasher.finish()
        };

        assert_eq!(hash1, hash2);
        assert_eq!(hash1, hash3);
        assert_eq!(hash2, hash3);
    }
}