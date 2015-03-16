/*!
This is a **prototype** implementation of a dynamic suffix array in external
memory. As such, its primary purpose is to minimize the number disk IOs, where
accessing any particular `Node` would count as a single disk IO. (Becuase we
will stipulate that a `Node` fits on a single page.)
*/
#![feature(io)]
#![allow(dead_code, unused_features, unused_imports, unused_variables)]

extern crate suffix;

use std::borrow::{Cow, IntoCow};
use std::fmt;
use std::iter;
use suffix::SuffixTable;

macro_rules! lg {
    ($($tt:tt)*) => ({
        use std::io::{Write, stderr};
        (writeln!(&mut stderr(), $($tt)*)).unwrap();
    });
}

/// A suffix database represented with a btree.
#[derive(Debug)]
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
#[derive(Debug)]
struct Node {
    /// Pointers to child nodes. Always has `suffixes.len() + 1` edges when
    /// `Node` is internal. Otherwise has `zero` edges when `Node` is a leaf.
    edges: Vec<KeyId>,
    /// Pointers to suffixes.
    suffixes: Vec<Suffix>,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

/// A document is contiguous sequence of UTF-8 bytes.
#[derive(Debug)]
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
            nodes: vec![Node::empty()],
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

#[derive(Debug)]
enum SearchResult {
    Found(NodeId, KeyId),
    InsertAt(NodeId, KeyId),
}

impl SearchResult {
    fn found(&self) -> bool {
        match *self {
            SearchResult::Found(_, _) => true,
            SearchResult::InsertAt(_, _) => false,
        }
    }

    fn ok(self) -> Option<(NodeId, KeyId)> {
        match self {
            SearchResult::Found(node_id, key_id) => Some((node_id, key_id)),
            _ => None,
        }
    }

    fn ids(&self) -> (NodeId, KeyId) {
        match *self {
            SearchResult::Found(node_id, key_id) => (node_id, key_id),
            SearchResult::InsertAt(node_id, key_id) => (node_id, key_id),
        }
    }
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
    fn search<'d, 's, S>(&'d self, needle: S) -> Suffixes<'d, 's>
            where S: IntoCow<'s, str> {
        let needle = needle.into_cow();
        let cur = self.search_start(&needle).ok();
        Suffixes {
            db: self,
            needle: needle,
            cur: cur,
        }
    }

    fn contains<'s, S>(&self, needle: S) -> bool where S: IntoCow<'s, str> {
        self.search(needle).next().is_some()
    }

    fn search_insert_at(&self, needle: &str) -> (NodeId, KeyId) {
        self.search_start(needle).ids()
    }

    fn search_start(&self, needle: &str) -> SearchResult {
        self.search_start_from(self.root, needle)
    }

    fn search_start_from(&self, nid: NodeId, needle: &str) -> SearchResult {
        let node = &self.nodes[nid];
        let mut kid: Option<KeyId> = None;
        for (i, suf) in node.suffixes.iter().enumerate() {
            if needle <= self.suffix(suf) {
                kid = Some(i);
                break
            }
        }
        let notfound = node.suffixes.len();
        match (node.is_leaf(), kid) {
            (true, None) => SearchResult::InsertAt(nid, notfound),
            (true, Some(kid)) => {
                if self.suffix(&node.suffixes[kid]).starts_with(needle) {
                    SearchResult::Found(nid, kid)
                } else {
                    SearchResult::InsertAt(nid, kid)
                }
            }
            (false, None) => {
                self.search_start_from(node.edges[notfound], needle)
            }
            (false, Some(kid)) => {
                self.search_start_from(node.edges[kid], needle)
            }
        }
    }

    fn next_suffix(&self, nid: NodeId, kid: KeyId) -> Option<(NodeId, KeyId)> {
        let node = &self.nodes[nid];
        if kid + 1 >= node.suffixes.len() {
            node.next.map(|nid| (nid, 0))
        } else {
            Some((nid, kid + 1))
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
        let (nid, kid) = self.search_insert_at(self.suffix(&suf));
        lg!("Inserting {:?} ('{}') at ({:?}, {:?})",
            suf, self.suffix(&suf), nid, kid);
        self.nodes[nid].suffixes.insert(kid, suf);
    }

    fn insert_at(&mut self, suf: Suffix, nid: NodeId) {
    }
}

impl Node {
    fn empty() -> Node {
        Node {
            edges: vec![],
            suffixes: vec![],
            prev: None,
            next: None,
        }
    }

    fn is_leaf(&self) -> bool {
        self.edges.is_empty()
    }
}

impl<'a, S> iter::FromIterator<S> for SufDB where S: IntoCow<'a, str> {
    fn from_iter<I>(docs: I) -> SufDB where I: iter::IntoIterator<Item=S> {
        let mut db = SufDB::new();
        db.extend(docs);
        db
    }
}

impl<'a, S> iter::Extend<S> for SufDB where S: IntoCow<'a, str> {
    fn extend<I>(&mut self, docs: I) where I: iter::IntoIterator<Item=S> {
        for doc in docs {
            self.insert(doc);
        }
    }
}

impl fmt::Debug for Suffix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Suffix({}, {})", self.docid, self.sufid)
    }
}

#[derive(Debug)]
struct Suffixes<'d, 's> {
    db: &'d SufDB,
    needle: Cow<'s, str>,
    cur: Option<(NodeId, KeyId)>,
}

impl<'d, 's> Iterator for Suffixes<'d, 's> {
    type Item = &'d Suffix;

    fn next(&mut self) -> Option<&'d Suffix> {
        if let Some((nid, kid)) = self.cur {
            let suf = &self.db.nodes[nid].suffixes[kid];
            if self.db.suffix(suf).starts_with(&self.needle) {
                self.cur = self.db.next_suffix(nid, kid);
                Some(&self.db.nodes[nid].suffixes[kid])
            } else {
                None
            }
        } else {
            None
        }
    }
}

mod tests {
    use std::borrow::IntoCow;
    use std::iter::{FromIterator, IntoIterator};
    use super::SufDB;

    fn createdb<'a, I>(docs: I) -> SufDB
            where I: IntoIterator,
                  <I as IntoIterator>::Item: IntoCow<'a, str> {
        FromIterator::from_iter(docs)
    }

    #[test]
    fn search_one() {
        let db = createdb(vec!["banana"]);
        lg!("{:?}", db);
        for suf in db.search("n") {
            lg!("{:?}: {:?}", suf, db.suffix(suf));
        }
    }

    #[test]
    #[ignore]
    fn scratch() {
        let mut db = SufDB::new();
        db.insert("banana");
        lg!("{:?}", db);
    }
}
