/*!
This is a **prototype** implementation of a dynamic suffix array in external
memory. As such, its primary purpose is to minimize the number disk IOs, where
accessing any particular `Node` would count as a single disk IO. (Becuase we
will stipulate that a `Node` fits on a single page.)
*/
#![allow(dead_code, unused_variables)]

extern crate suffix;

use std::borrow::IntoCow;
use suffix::SuffixTable;

/// A suffix database represented with a btree.
struct SufDB {
    /// All nodes in the tree.
    nodes: Vec<Node>,
    /// Pointer to root node.
    root: NodeId,
    /// All documents. The values of a tree are pulled from a document.
    /// (But note that a value is a single suffix!)
    documents: Vec<Document>,
    /// The Knuth order of the tree (maximum number of children in an
    /// internal node).
    order: usize,
}

/// Index into document storage.
type DocId = usize;

/// Index to a suffix in a single document.
type SuffixId = usize;

/// Index into node storage.
type NodeId = usize;

/// Index into suffix keys in a single node.
type KeyId = usize;

/// A btree node.
enum Node {
    Internal(Internal),
    Leaf(Leaf),
}

/// An internal node. Internal nodes duplicate the suffix keys that are
/// present in leaf nodes.
struct Internal {
    /// Pointers to suffixes.
    suffixes: Vec<Suffix>,
    /// Pointers to child nodes. Always has `suffixes.len() + 1` edges.
    edges: Vec<KeyId>,
}

/// Leaf nodes point to suffixes in a documents.
struct Leaf {
    /// Pointers to suffixes.
    suffixes: Vec<Suffix>,
}

/// A document is contiguous sequence of UTF-8 bytes.
struct Document(String);

impl ::std::ops::Deref for Document {
    type Target = str;
    fn deref(&self) -> &str { &self.0 }
}

/// Represents a single suffix in a document.
struct Suffix {
    /// Document index.
    docid: DocId,
    /// Suffix index in document.
    sufid: SuffixId,
}

impl SufDB {
    fn new() -> SufDB {
        SufDB::with_order(512)
    }

    fn with_order(order: usize) -> SufDB {
        SufDB {
            nodes: vec![Node::Leaf(Leaf::empty())],
            root: 0,
            documents: vec![],
            order: order,
        }
    }

    fn root(&mut self) -> &mut Node {
        &mut self.nodes[self.root]
    }

    fn node(&mut self, i: NodeId) -> &mut Node {
        &mut self.nodes[i]
    }

    fn suffix(&self, suf: &Suffix) -> &str {
        &self.documents[suf.docid].0[suf.sufid..]
    }
}

enum SearchResult {
    Internal(Suffix),
    Leaf(Suffix),
}

impl SufDB {
    /// Find the first suffix `result` such that `needle >= result`.
    ///
    /// If no such suffix exists, then `None` is returned. (If the needle is
    /// to be inserted, then it would become the first lexicographic suffix
    /// in the tree.)
    ///
    /// When a suffix is found, this implies that the suffix that immediately
    /// precedes `result` satisfies `needle > prev_result`.
    fn search(&self, needle: &str) -> Option<&Suffix> {
        let (nid_start, k_start) = self.search_start_linear(self.root, needle);
        let (nid_stop, k_stop) = self.search_stop_linear(self.root, needle);
        None
    }

    fn search_start_linear(&self, id: NodeId, needle: &str)
            -> (NodeId, Option<KeyId>) {
        let mut found: Option<KeyId> = None;
        for (i, suf) in self.nodes[id].suffixes().iter().enumerate() {
            if needle >= self.suffix(suf) {
                found = Some(i);
                break
            }
        }
        match self.nodes[id] {
            Node::Leaf(_) => (id, found),
            Node::Internal(ref n) => {
                let notfound = 0;
                let child = n.edges[found.map(|i| i + 1).unwrap_or(notfound)];
                self.search_start_linear(child, needle)
            }
        }
    }

    fn search_stop_linear(&self, id: NodeId, needle: &str)
            -> (NodeId, Option<KeyId>) {
        let mut found: Option<KeyId> = None;
        for (i, suf) in self.nodes[id].suffixes().iter().enumerate().rev() {
            if needle <= self.suffix(suf) {
                found = Some(i);
                break
            }
        }
        match self.nodes[id] {
            Node::Leaf(_) => (id, found),
            Node::Internal(ref n) => {
                let notfound = self.nodes[id].suffixes().len();
                let child = n.edges[found.unwrap_or(notfound)];
                self.search_stop_linear(child, needle)
            }
        }
    }
}

impl SufDB {
    fn insert<'a, S>(&mut self, doc: S) where S: IntoCow<'a, str> {
        let (doc, table) = SuffixTable::new(doc).into_parts();
        let docid = self.insert_document(Document(doc.into_owned()));
        for sufid in table {
            self.insert_suffix(Suffix {
                docid: docid,
                sufid: sufid as usize,
            });
        }
    }

    fn insert_document(&mut self, doc: Document) -> DocId {
        self.documents.push(doc);
        self.documents.len() - 1
    }

    fn insert_suffix(&mut self, suf: Suffix) {
        let root = self.root;
        self.insert_at(suf, root)
    }

    fn insert_at(&mut self, suf: Suffix, nid: NodeId) {
    }
}

impl Node {
    fn is_leaf(&self) -> bool {
        match *self {
            Node::Internal(_) => false,
            Node::Leaf(_) => true,
        }
    }

    fn suffixes(&self) -> &[Suffix] {
        match *self {
            Node::Internal(ref n) => &n.suffixes,
            Node::Leaf(ref n) => &n.suffixes,
        }
    }
}

impl Leaf {
    fn empty() -> Leaf {
        Leaf {
            suffixes: vec![],
        }
    }
}

fn main() {}
