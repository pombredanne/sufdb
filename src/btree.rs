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

type DocId = usize;
type SuffixId = usize;
type NodeId = usize;

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
    edges: Vec<usize>,
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
    fn search_start(&self, needle: &str) -> Option<&Suffix> {
    }

    fn search_node_linear<F>(
        &self,
        id: NodeId,
        needle: &str,
        pred: F,
    ) -> Option<&Suffix> where F: Fn(&str) -> bool {
        let mut i: usize = 0;
        for (i, suf) in self.nodes[id].suffixes().iter().enumerate() {
            if pred(self.suffix(suf)) {
                found = i;
                break;
            }
        }
        if self.nodes[id].is_leaf() {
            Some(&self.nodes[i].suffixes()[i])
        } else {
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
            Node::Internal => false,
            Node::Leaf => true,
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
