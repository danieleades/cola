use std::fmt::Debug;
use std::ops::Range;

use cola::CrdtEdit;

pub struct Replica<B: Buffer> {
    buffer: B,
    crdt: cola::Replica,
}

impl<B: Buffer + Clone> Clone for Replica<B> {
    fn clone(&self) -> Self {
        Self { buffer: self.buffer.clone(), crdt: self.crdt.clone() }
    }
}

impl<B: Buffer + Debug> Debug for Replica<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replica")
            .field("buffer", &self.buffer)
            .field("crdt", &self.crdt.debug())
            .finish()
    }
}

impl<B: Buffer + for<'a> PartialEq<&'a str>> PartialEq<&str> for Replica<B> {
    fn eq(&self, rhs: &&str) -> bool {
        self.buffer == rhs
    }
}

impl<B: Buffer + for<'a> PartialEq<&'a str>> PartialEq<Replica<B>> for &str {
    fn eq(&self, rhs: &Replica<B>) -> bool {
        rhs.buffer == self
    }
}

impl<B: Buffer> Replica<B> {
    pub fn delete(&mut self, byte_range: Range<usize>) -> CrdtEdit {
        self.buffer.delete(byte_range.clone());
        self.crdt.deleted(byte_range)
    }

    pub fn insert<T: Into<String>>(
        &mut self,
        byte_offset: usize,
        text: T,
    ) -> CrdtEdit {
        let text = text.into();
        self.buffer.insert(byte_offset, text.as_str());
        self.crdt.inserted(byte_offset, text.len())
    }

    pub fn merge(&mut self, crdt_edit: &CrdtEdit) {
        if let Some(edit) = self.crdt.merge(crdt_edit.clone()) {
            self.buffer.replace(edit.range, "");
        }
    }

    pub fn new<T: Into<B>>(text: T) -> Self {
        let buffer = text.into();
        let crdt = cola::Replica::new(buffer.measure());
        Self { buffer, crdt }
    }
}

impl<B: Buffer + Debug> Replica<B> {
    pub fn as_btree(&self) -> DebugAsBtree<'_, B> {
        DebugAsBtree(self)
    }
}

pub trait Buffer {
    fn measure(&self) -> u64;

    fn insert(&mut self, byte_offset: usize, text: &str);

    fn delete(&mut self, byte_range: Range<usize>);

    fn replace(&mut self, byte_range: Range<usize>, text: &str) {
        let start = byte_range.start;
        self.delete(byte_range);
        self.insert(start, text);
    }
}

impl Buffer for String {
    fn measure(&self) -> u64 {
        self.len() as _
    }

    fn insert(&mut self, byte_offset: usize, text: &str) {
        self.insert_str(byte_offset, text);
    }

    fn delete(&mut self, byte_range: Range<usize>) {
        self.replace_range(byte_range, "");
    }

    fn replace(&mut self, byte_range: Range<usize>, text: &str) {
        self.replace_range(byte_range, text);
    }
}

pub struct DebugAsBtree<'a, B: Buffer + Debug>(&'a Replica<B>);

impl<B: Buffer + Debug> Debug for DebugAsBtree<'_, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let replica = self.0;

        f.debug_struct("Replica")
            .field("buffer", &replica.buffer)
            .field("crdt", &replica.crdt.debug_as_btree())
            .finish()
    }
}
