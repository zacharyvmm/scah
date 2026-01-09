#[derive(Debug)]
pub enum Selection<M, L> {
    First(M),
    All(L),
}
