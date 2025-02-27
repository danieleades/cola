# 🥤 cola

[![CI]](https://github.com/nomad/cola/actions)
[![Latest version]](https://crates.io/crates/cola-crdt)
[![Docs badge]][docs]

[CI]: https://github.com/nomad/cola/actions/workflows/ci.yml/badge.svg
[Latest version]: https://img.shields.io/crates/v/cola-crdt.svg
[Docs badge]: https://docs.rs/cola-crdt/badge.svg

cola is a Conflict-free Replicated Data Type specialized for real-time
collaborative editing of plain text documents.

It allows multiple peers on a distributed network to concurrently edit the same
text document, making sure that they all converge to the same final state
without relying on a central server to coordinate the edits.

Check out [the docs][docs] to learn about cola's API, or [this blog post][cola]
for a deeper dive into its design and implementation.

## Example usage

```rust
use std::ops::Range;

use cola::{Deletion, Replica, ReplicaId};

struct Document {
    buffer: String,
    crdt: Replica,
}

struct Insertion {
    text: String,
    crdt: cola::Insertion,
}

impl Document {
    fn new<S: Into<String>>(text: S, replica_id: ReplicaId) -> Self {
        let buffer = text.into();
        let crdt = Replica::new(replica_id, buffer.len());
        Document { buffer, crdt }
    }

    fn fork(&self, new_replica_id: ReplicaId) -> Self {
        let crdt = self.crdt.fork(new_replica_id);
        Document { buffer: self.buffer.clone(), crdt }
    }

    fn insert<S: Into<String>>(
        &mut self,
        insert_at: usize,
        text: S,
    ) -> Insertion {
        let text = text.into();
        self.buffer.insert_str(insert_at, &text);
        let insertion = self.crdt.inserted(insert_at, text.len());
        Insertion { text, crdt: insertion }
    }

    fn delete(&mut self, range: Range<usize>) -> Deletion {
        self.buffer.replace_range(range.clone(), "");
        self.crdt.deleted(range)
    }

    fn integrate_insertion(&mut self, insertion: Insertion) {
        if let Some(offset) = self.crdt.integrate_insertion(&insertion.crdt) {
            self.buffer.insert_str(offset, &insertion.text);
        }
    }

    fn integrate_deletion(&mut self, deletion: Deletion) {
        let ranges = self.crdt.integrate_deletion(&deletion);
        for range in ranges.into_iter().rev() {
            self.buffer.replace_range(range, "");
        }
    }
}

fn main() {
    let mut peer_1 = Document::new("Hello, world", 1);
    let mut peer_2 = peer_1.fork(2);

    let delete_comma = peer_1.delete(5..6);
    let insert_exclamation = peer_2.insert(12, "!");

    peer_1.integrate_insertion(insert_exclamation);
    peer_2.integrate_deletion(delete_comma);

    assert_eq!(peer_1.buffer, "Hello world!");
    assert_eq!(peer_2.buffer, "Hello world!");
}
```

# A note on the crate's naming scheme

cola's `package.name` is `cola-crdt`, while its `lib.name` is simply `cola`.
This is because the package name has to be unique to be published to
[crates.io], but unfortunately `cola` is already taken by a crate squatter.

What this means practically for you, the user of the library, is that you
should import cola as `cola-crdt` in your `Cargo.toml`, and `use` it as
`cola` in your source code.

For example:

```toml
# Cargo.toml
[dependencies]
cola-crdt = "0.1"
```

```rust
// main.rs
use cola::Replica;

fn main() {
    println!("{:?}", Replica::new(1, 42));
}
```

[docs]: https://docs.rs/cola-crdt
[cola]: https://nomad.foo/blog/cola
[crates.io]: https://crates.io
