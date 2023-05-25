use core::ops::Range;

use crate::node::{Inode, Metric, Node, Summarize};

#[derive(Clone)]
pub struct Tree<const ARITY: usize, Leaf: Summarize> {
    root: Node<ARITY, Leaf>,
}

impl<const ARITY: usize, Leaf: Summarize> core::fmt::Debug
    for Tree<ARITY, Leaf>
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.root, f)
    }
}

impl<const ARITY: usize, Leaf: Summarize> From<Leaf> for Tree<ARITY, Leaf> {
    #[inline]
    fn from(leaf: Leaf) -> Self {
        Self { root: Node::Leaf(leaf) }
    }
}

impl<const ARITY: usize, Leaf: Summarize> Tree<ARITY, Leaf> {
    #[inline]
    fn measure<M: Metric<Leaf>>(&self) -> M {
        M::measure_summary(&self.summary())
    }

    #[inline]
    fn replace_root<F>(&mut self, replace_with: F)
    where
        F: FnOnce(Node<ARITY, Leaf>) -> Node<ARITY, Leaf>,
    {
        let dummy_node = Node::Internal(Inode::empty());
        let old_root = core::mem::replace(&mut self.root, dummy_node);
        self.root = replace_with(old_root);
    }

    #[inline]
    fn root_mut(&mut self) -> &mut Node<ARITY, Leaf> {
        &mut self.root
    }

    #[inline]
    pub fn summary(&self) -> Leaf::Summary {
        self.root.summary()
    }
}

mod tree_insert {
    use super::*;
    use crate::{Fragment, Replica};

    type Tree = super::Tree<{ Replica::arity() }, Fragment>;
    type Node = super::Node<{ Replica::arity() }, Fragment>;
    type Inode = super::Inode<{ Replica::arity() }, Fragment>;

    impl Tree {
        #[inline]
        pub fn insert<M, F>(&mut self, insert_at: M, insert_with: F)
        where
            M: Metric<Fragment>,
            F: FnOnce(M, &mut Fragment) -> (Fragment, Option<Fragment>),
        {
            debug_assert!(insert_at <= self.measure::<M>());

            let root = match &mut self.root {
                Node::Internal(inode) => inode,

                Node::Leaf(fragment) => {
                    let (leaf, extra) = insert_with(M::zero(), fragment);

                    self.replace_root(|old_root| {
                        let leaf = Node::Leaf(leaf);

                        let children = if let Some(extra) = extra {
                            vec![old_root, leaf, Node::Leaf(extra)]
                        } else {
                            vec![old_root, leaf]
                        };

                        Node::from_children(children)
                    });

                    return;
                },
            };

            if let Some(extra) =
                tree_insert::insert(root, insert_at, insert_with)
                    .map(Node::Internal)
            {
                self.replace_root(|old_root| {
                    Node::from_children(vec![old_root, extra])
                });
            }
        }
    }

    #[inline]
    pub(super) fn insert<M, F>(
        inode: &mut Inode,
        mut insert_at: M,
        insert_with: F,
    ) -> Option<Inode>
    where
        M: Metric<Fragment>,
        F: FnOnce(M, &mut Fragment) -> (Fragment, Option<Fragment>),
    {
        let mut offset = M::zero();

        for (idx, child) in inode.children_mut().iter_mut().enumerate() {
            let child_measure = child.measure::<M>();

            offset += child_measure;

            let child_contains_insert = offset >= insert_at;

            if child_contains_insert {
                offset -= child_measure;
                insert_at -= offset;

                if child.is_internal() {
                    let extra = inode.with_child_mut(idx, |child| {
                        let inode = child.as_internal_mut();
                        insert(inode, insert_at, insert_with)
                    });

                    return extra.and_then(|e| {
                        inode.insert(idx + 1, Node::Internal(e))
                    });
                } else {
                    let (leaf, extra) = inode.with_child_mut(idx, |child| {
                        let fragment = child.as_leaf_mut();
                        insert_with(offset, fragment)
                    });

                    let leaf = Node::Leaf(leaf);

                    let offset = idx + 1;

                    return if let Some(extra) = extra.map(Node::Leaf) {
                        inode.insert_two(offset, leaf, offset, extra)
                    } else {
                        inode.insert(offset, leaf)
                    };
                }
            }
        }

        unreachable!();
    }
}

mod tree_delete {
    use super::*;
    use crate::{Fragment, Replica};

    type Tree = super::Tree<{ Replica::arity() }, Fragment>;
    type Node = super::Node<{ Replica::arity() }, Fragment>;
    type Inode = super::Inode<{ Replica::arity() }, Fragment>;

    impl Tree {
        #[inline]
        pub fn delete(&mut self, delete_range: Range<usize>) {
            let root = match self.root_mut() {
                Node::Internal(inode) => inode,

                Node::Leaf(fragment) => {
                    let (deleted, rest) = fragment.delete_range(delete_range);

                    if let Some(deleted) = deleted.map(Node::Leaf) {
                        self.replace_root(|old_root| {
                            let children =
                                if let Some(rest) = rest.map(Node::Leaf) {
                                    vec![old_root, deleted, rest]
                                } else {
                                    vec![old_root, deleted]
                                };

                            Node::from_children(children)
                        })
                    }

                    return;
                },
            };

            if let Some(extra) =
                tree_delete::delete(root, delete_range).map(Node::Internal)
            {
                self.replace_root(|old_root| {
                    Node::from_children(vec![old_root, extra])
                });
            }
        }
    }

    impl Node {
        fn delete(&mut self) {
            match self {
                Node::Internal(inode) => {
                    inode.summary_mut().len = 0;
                },

                Node::Leaf(fragment) => fragment.delete(),
            }
        }
    }

    #[inline]
    fn delete(
        inode: &mut Inode,
        mut delete_range: Range<usize>,
    ) -> Option<Inode> {
        let mut offset = 0;

        for (idx, child) in inode.children().iter().enumerate() {
            let child_len = child.summary().len;

            offset += child_len;

            let child_contains_range_start = offset >= delete_range.start;

            if !child_contains_range_start {
                continue;
            }

            let child_contains_range_end = offset >= delete_range.end;

            if child_contains_range_end {
                offset -= child_len;
                delete_range.start -= offset;
                delete_range.end -= offset;

                if child.is_internal() {
                    let extra = inode.with_child_mut(idx, |child| {
                        let inode = child.as_internal_mut();
                        delete(inode, delete_range)
                    });

                    return extra.and_then(|e| {
                        inode.insert(idx + 1, Node::Internal(e))
                    });
                } else {
                    let (deleted, rest) = inode.with_child_mut(idx, |child| {
                        let fragment = child.as_leaf_mut();
                        fragment.delete_range(delete_range)
                    });

                    let deleted = deleted.map(Node::Leaf)?;

                    let offset = idx + 1;

                    return if let Some(rest) = rest.map(Node::Leaf) {
                        inode.insert_two(offset, deleted, offset, rest)
                    } else {
                        inode.insert(offset, deleted)
                    };
                }
            } else {
                return delete_range_in_deepest(inode, delete_range);
            }
        }

        unreachable!();
    }

    #[inline]
    fn delete_range_in_deepest(
        inode: &mut Inode,
        delete_range: Range<usize>,
    ) -> Option<Inode> {
        let mut start_idx = 0;

        let mut end_idx = 0;

        let mut extra_from_start = None;

        let mut extra_from_end = None;

        let mut children = inode.children_mut().iter_mut().enumerate();

        let mut offset = 0;

        for (idx, child) in children.by_ref() {
            let child_len = child.summary().len;

            offset += child_len;

            if offset >= delete_range.start {
                start_idx = idx;
                let delete_from = delete_range.start + child_len - offset;
                extra_from_start = something_start(child, delete_from);
                break;
            }
        }

        for (idx, child) in children {
            let child_len = child.summary().len;

            offset += child_len;

            if offset >= delete_range.end {
                end_idx = idx;
                let delete_up_to = delete_range.end + child_len - offset;
                extra_from_end = something_end(child, delete_up_to);
                break;
            } else {
                child.delete()
            }
        }

        *inode.summary_mut() = inode.summarize();

        match (extra_from_start, extra_from_end) {
            (Some(start), Some(end)) => {
                inode.insert_two(start_idx + 1, start, end_idx + 1, end)
            },

            (Some(start), None) => inode.insert(start_idx + 1, start),

            (None, Some(end)) => inode.insert(end_idx + 1, end),

            (None, None) => None,
        }
    }

    #[inline]
    fn something_start(node: &mut Node, delete_from: usize) -> Option<Node> {
        let inode = match node {
            Node::Internal(inode) => inode,

            Node::Leaf(fragment) => {
                return fragment.delete_from(delete_from).map(Node::Leaf);
            },
        };

        let mut start_idx = 0;

        let mut extra = None;

        let mut offset = 0;

        let mut children = inode.children_mut().iter_mut();

        for (idx, child) in children.by_ref().enumerate() {
            let child_len = child.summary().len;

            if offset + child_len >= delete_from {
                start_idx = idx;
                extra = something_start(child, delete_from - offset);
                break;
            } else {
                offset += child_len;
            }
        }

        for child in children {
            child.delete()
        }

        *inode.summary_mut() = inode.summarize();

        extra.and_then(|e| inode.insert(start_idx + 1, e)).map(Node::Internal)
    }

    #[inline]
    fn something_end(node: &mut Node, delete_up_to: usize) -> Option<Node> {
        let inode = match node {
            Node::Internal(inode) => inode,

            Node::Leaf(fragment) => {
                return fragment.delete_up_to(delete_up_to).map(Node::Leaf);
            },
        };

        let mut end_idx = 0;

        let mut extra = None;

        let mut offset = 0;

        for (idx, child) in inode.children_mut().iter_mut().enumerate() {
            let child_len = child.summary().len;

            if offset + child_len >= delete_up_to {
                end_idx = idx;
                extra = something_end(child, delete_up_to - offset);
                break;
            } else {
                child.delete();
                offset += child_len;
            }
        }

        *inode.summary_mut() = inode.summarize();

        extra.and_then(|e| inode.insert(end_idx + 1, e)).map(Node::Internal)
    }
}