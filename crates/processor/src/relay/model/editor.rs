use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Default, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub include: String,
    #[serde(default)]
    pub exclude: String,
    #[serde(default)]
    pub find: String,
    #[serde(default)]
    pub replace: String,
    #[serde(default)]
    pub deduplicate: bool,
    #[serde(default)]
    pub nodes: Vec<NodeEdit>,
    #[serde(default)]
    pub pipeline: Vec<super::pipeline::Step>,
}

impl Options {
    /// Any configured edit requires binding to the reviewed source, even if disabled.
    pub fn is_active(&self) -> bool {
        !self.include.is_empty()
            || !self.exclude.is_empty()
            || !self.find.is_empty()
            || !self.replace.is_empty()
            || self.deduplicate
            || !self.nodes.is_empty()
            || !self.pipeline.is_empty()
    }
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
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
    pub stages: Vec<super::pipeline::StageReport>,
}
