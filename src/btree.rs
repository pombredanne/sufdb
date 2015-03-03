#![allow(dead_code)]

/// A suffix database represented with a btree.
struct SufDB {
    /// All nodes in the tree. `0` is root and is guaranteed to always be
    /// present (but possibly empty).
    nodes: Vec<Node>,
    /// All documents. The values of a tree are pulled from a document.
    /// (But note that a value is a single suffix!)
    documents: Vec<Document>,
    /// The Knuth order of the tree (maximum number of children in an
    /// internal node).
    order: usize,
}

/// A btree node.
struct Node {
    /// Pointers to suffixes in a particular document.
    suffixes: Vec<Suffix>,
    /// Points to child nodes. `None` when `Node` is a leaf.
    /// When not `None`, it's an internal node and always has
    /// `suffixes.len() + 1` edges.
    edges: Option<Vec<usize>>,
}

/// A document is contiguous sequence of UTF-8 bytes.
struct Document(String);

/// Represents a single suffix in a document.
struct Suffix {
    /// Document index.
    doci: usize,
    /// Suffix index in document.
    sufi: usize,
}

impl SufDB {
    fn root(&self) -> &Node {
        &self.nodes[0]
    }

    fn suffix(&self, suf: &Suffix) -> &str {
        &self.documents[suf.doci].0[suf.sufi..]
    }
}

fn main() {}
