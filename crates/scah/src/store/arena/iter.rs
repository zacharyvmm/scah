use super::Arena;

pub struct ArenaIterator<'arena, T: 'arena, I> {
    arena: &'arena Arena<T, I>,
    cursor: I,
}

impl<'arena, T: 'arena, I> ArenaIterator<'arena, T, I> {
    pub fn new(arena: &'arena Arena<T, I>, cursor: I) -> Self {
        Self { arena, cursor }
    }
}

pub(crate) trait Nullable {
    fn is_null(&self) -> bool;
}
pub(crate) trait Node<I> {
    fn next_sibling(&self) -> Option<I>;
}

impl<'arena, T: 'arena + Node<I>, I: Default + Nullable + Into<usize> + Copy> Iterator
    for ArenaIterator<'arena, T, I>
{
    type Item = &'arena T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.is_null() {
            return None;
        }
        let item = &self.arena[self.cursor];
        self.cursor = item.next_sibling().unwrap_or_default();
        Some(item)
    }
}
