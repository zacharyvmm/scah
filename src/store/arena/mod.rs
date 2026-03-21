use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut};
pub mod id;
mod iter;
pub(crate) use iter::Node;

#[derive(Debug, PartialEq)]
pub struct Arena<T, I> {
    pub(super) inner: Vec<T>,
    _marker: PhantomData<I>,
}

impl<T, I: From<usize>> Arena<T, I> {
    pub fn new() -> Self {
        Self {
            inner: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    pub unsafe fn index_of(&self, item: &T) -> I {
        let list_ptr_range = self.inner.as_ptr_range();
        let ptr = std::ptr::from_ref(item);
        assert!(list_ptr_range.contains(&ptr));

        let index = unsafe { ptr.offset_from_unsigned(list_ptr_range.start) };
        I::from(index)
    }

    pub fn iter_from<'a>(&'a self, from: I) -> iter::ArenaIterator<'a, T, I> {
        iter::ArenaIterator::new(self, from)
    }
}

impl<T, I: From<usize>> Default for Arena<T, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, I> Deref for Arena<T, I> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, I> DerefMut for Arena<T, I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T, I: Into<usize>> Index<I> for Arena<T, I> {
    type Output = T;

    fn index(&self, index: I) -> &Self::Output {
        &self.inner[index.into()]
    }
}

impl<T, I: Into<usize>> IndexMut<I> for Arena<T, I> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.inner[index.into()]
    }
}
