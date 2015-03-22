/*!
This is a **prototype** implementation of a dynamic suffix array in external
memory. As such, its primary purpose is to minimize the number disk IOs, where
accessing any particular `Node` would count as a single disk IO. (Becuase we
will stipulate that a `Node` fits on a single page.)
*/

#![feature(collections)]
#![allow(dead_code, unused_imports, unused_variables)]

extern crate suffix;

use std::borrow::{Cow, IntoCow};
use std::fmt;
use std::iter::{self, repeat};
use suffix::SuffixTable;

macro_rules! lg {
    ($($tt:tt)*) => ({
        use std::io::{Write, stderr};
        (writeln!(&mut stderr(), $($tt)*)).unwrap();
    });
}

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
#[derive(Debug)]
struct Node {
    /// Pointers to child nodes. Always has `suffixes.len() + 1` edges when
    /// `Node` is internal. Otherwise has `zero` edges when `Node` is a leaf.
    edges: Vec<KeyId>,
    /// Pointers to suffixes.
    suffixes: Vec<Suffix>,
    parent: Option<NodeId>,
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
#[derive(Clone, Eq, Hash, PartialEq)]
struct Suffix {
    /// Document index.
    docid: DocId,
    /// Suffix index in document.
    sufid: SuffixId,
}

impl Suffix {
    fn new(docid: DocId, sufid: SuffixId) -> Suffix {
        Suffix { docid: docid, sufid: sufid }
    }
}

impl SufDB {
    fn new() -> SufDB {
        SufDB::with_order(14)
    }

    fn with_order(order: usize) -> SufDB {
        assert!(order > 2);
        SufDB {
            nodes: vec![Node::empty()],
            root: 0,
            documents: vec![],
            order: order,
        }
    }

    fn max_children(&self) -> usize { self.order }
    fn max_keys(&self) -> usize { self.order - 1 }

    fn is_root(&self, nid: NodeId) -> bool { self.root == nid }

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
        match (node.is_leaf(), self.search_scan(nid, needle)) {
            (true, Err(kid)) => SearchResult::InsertAt(nid, kid),
            (true, Ok(kid)) => {
                if self.suffix(&node.suffixes[kid]).starts_with(needle) {
                    SearchResult::Found(nid, kid)
                } else {
                    SearchResult::InsertAt(nid, kid)
                }
            }
            (false, Err(kid)) | (false, Ok(kid)) => {
                self.search_start_from(node.edges[kid], needle)
            }
        }
    }

    fn search_scan(&self, nid: NodeId, needle: &str) -> Result<KeyId, KeyId> {
        let node = &self.nodes[nid];
        let mut kid: Result<KeyId, KeyId> = Err(node.suffixes.len());
        for (i, suf) in node.suffixes.iter().enumerate() {
            if needle <= self.suffix(suf) {
                kid = Ok(i);
                break
            }
        }
        kid
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
        self.nodes[nid].suffixes.insert(kid, suf);
        // if self.nodes[nid].suffixes.len() > self.max_keys() {
            // let median = self.split(nid);
        // }
    }

    // fn split(&mut self, nid: NodeId) -> Suffix {
        // let new = self.nodes[nid].split();
    // }
}

impl Node {
    fn empty() -> Node {
        Node {
            edges: vec![],
            suffixes: vec![],
            parent: None,
            next: None,
        }
    }

    fn is_leaf(&self) -> bool {
        self.edges.is_empty()
    }

    fn split(&mut self) -> Node {
        let median = self.suffixes.len() / 2;
        let split_sufs = self.suffixes.split_off(median);
        let split_edges = if self.is_leaf() {
            vec![]
        } else {
            self.edges.split_off(median)
        };
        Node {
            edges: split_edges,
            suffixes: split_sufs,
            parent: self.parent,
            next: None,
        }
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

impl fmt::Debug for SufDB {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        macro_rules! w { ($($tt:tt)*) => (try!(writeln!(f, $($tt)*));); }
        macro_rules! div {
            () => (div!(79));
            ($n:expr) => ({
                w!("{}", repeat('-').take($n).collect::<String>());
            });
        }

        div!();
        w!("order: {}", self.order);
        w!("#documents: {}", self.documents.len());
        w!("#nodes: {}", self.nodes.len());
        div!(39);
        for doc in &self.documents { w!("{:?}", doc); }
        div!(39);
        for (i, n) in self.nodes.iter().enumerate() {
            if i > 0 { div!(15); }
            if self.root == i {
                w!("id: {} (root)", i);
            } else {
                w!("id: {}", i);
            };
            w!("parent: {:?}", n.parent);
            w!("next: {:?}", n.next);
            w!("edges: {:?}", n.edges);
            for (i, s) in n.suffixes.iter().enumerate() {
                w!("{:4}: {:?}: {}", i, s, self.suffix(s));
            }
        }
        div!(); Ok(())
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
        if self.needle.is_empty() {
            return None;
        }
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
    use std::collections::HashSet;
    use std::fmt::Debug;
    use std::hash::Hash;
    use std::iter::{FromIterator, IntoIterator};
    use super::{SufDB, Suffix, DocId, SuffixId};

    fn createdb<'a, I>(docs: I) -> SufDB
            where I: IntoIterator,
                  <I as IntoIterator>::Item: IntoCow<'a, str> {
        FromIterator::from_iter(docs)
    }

    fn suf(d: DocId, s: SuffixId) -> Suffix { Suffix::new(d, s) }

    fn assert_set_eq<T, A, B>(a: A, b: B)
            where T: Debug + Eq  + Hash,
                  A: IntoIterator<Item=T>,
                  B: IntoIterator<Item=T> {
        let s1: HashSet<<A as IntoIterator>::Item> = a.into_iter().collect();
        let s2: HashSet<<B as IntoIterator>::Item> = b.into_iter().collect();
        assert_eq!(s1, s2);
    }

    fn assert_search(db: &SufDB, search: &str, sufs: Vec<(DocId, SuffixId)>) {
        assert_set_eq(db.search(search).map(|s| s.clone()),
                      sufs.into_iter().map(|(d, s)| suf(d, s)));
    }

    #[test]
    fn search_empty_db() {
        let db = SufDB::new();
        assert!(!db.contains(""));
        assert!(!db.contains("a"));
    }

    #[test]
    fn search_one_letter() {
        let db = createdb(vec!["a"]);
        assert!(db.contains("a"));
        assert_search(&db, "a", vec![(0, 0)]);
    }

    #[test]
    fn search_one() {
        let db = createdb(vec!["banana"]);
        assert!(db.contains("ana"));
        assert!(db.contains("a"));
        assert!(!db.contains(""));
        assert!(!db.contains("z"));
    }

    #[test]
    fn search_two() {
        let db = createdb(vec!["banana", "apple"]);
        assert!(db.contains("ana"));
        assert!(db.contains("a"));
        assert!(db.contains("apple"));
        assert!(db.contains("banana"));
        assert!(!db.contains(""));
        assert!(!db.contains("z"));
        assert!(!db.contains("aa")); // make sure suffixes are separate!
    }

    #[test]
    fn search_two_similar() {
        let db = createdb(vec!["maple", "apple"]);
        assert_search(&db, "ple", vec![(0, 2), (1, 2)]);
    }

    #[test]
    fn search_two_snowmen() {
        let db = createdb(vec!["☃abc☃", "apple"]);
        assert_search(&db, "☃", vec![(0, 0), (0, 6)]);
    }

    #[test]
    fn scratch() {
        let mut db = SufDB::new();
        db.insert("banana");
        db.insert("apple");
        db.insert("orange");
        lg!("{:?}", db);
    }
}
