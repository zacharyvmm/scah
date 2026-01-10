// Reduce Cloning by only cloning when more than a single fsm want's it
// Also the added benefit is that I can remove duplicate saving

#[derive(Debug)]
pub struct Fsm<E> {
    pub parent: *mut E,
    pub section: usize,

    // None means it's the main fsm
    // Some means it's scoped fsm index
    pub index: Option<usize>,    
}

#[derive(Debug)]
pub struct Reserve<E> {
    pub list: Vec<Fsm<E>>,
    section: usize,
}
impl<E> Reserve<E> {
    pub fn new() -> Self {
        Self {
            list: vec![],
            section: 0,
        }
    }

    pub fn push(&mut self, parent: *mut E, index: Option<usize>) {
        if !self.list.iter().any(|i| i.parent == parent) {
            let fsm = Fsm{parent, section: self.section, index};
            self.list.push(fsm);
        }
    }
 
    #[inline]
    pub fn set_section(&mut self, section: usize) {
        self.section = section;
    }
}