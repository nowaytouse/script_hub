use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct Options {
    pub include: String,
    pub exclude: String,
    pub find: String,
    pub replace: String,
    pub deduplicate: bool,
    pub nodes: Vec<NodeEdit>,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct NodeEdit {
    pub index: usize,
    pub included: bool,
    pub name: Option<String>,
}

pub struct Rendered {
    pub content: Zeroizing<String>,
    pub retained: Vec<usize>,
    pub excluded: usize,
    pub duplicates: usize,
    pub duplicate_skipped: usize,
}
