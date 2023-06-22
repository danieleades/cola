use alloc::collections::VecDeque;
use core::ops::RangeBounds;

use uuid::Uuid;

use crate::*;

/// TODO: docs
const ARITY: usize = 32;

/// TODO: docs
pub struct Replica {
    /// TODO: docs
    id: ReplicaId,

    /// TODO: docs
    insertion_runs: Gtree<ARITY, InsertionRun>,

    /// TODO: docs
    // run_indexes: RunIdRegistry,

    /// TODO: docs
    character_ts: CharacterTimestamp,

    /// TODO: docs
    lamport_clock: LamportClock,

    /// TODO: docs
    pending: VecDeque<CrdtEdit>,
}

impl core::fmt::Debug for Replica {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // In the public Debug we just print the ReplicaId to avoid leaking
        // our internals.
        //
        // During development the `Replica::debug()` method (which is public
        // but hidden from the API) can be used to obtain a more useful
        // representation.
        f.debug_tuple("Replica").field(&self.id.0).finish()
    }
}

impl Default for Replica {
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}

/// TODO: docs
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReplicaId(Uuid);

impl core::fmt::Debug for ReplicaId {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ReplicaId({:x})", self.as_u32())
    }
}

impl ReplicaId {
    /// TODO: docs
    pub fn as_u32(&self) -> u32 {
        self.0.as_fields().0
    }

    /// Creates a new, randomly generated [`ReplicaId`].
    #[inline(always)]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the "nil" id, i.e. the id whose bytes are all zeros.
    ///
    /// This is used to form the [`EditId`] of the first edit run and should
    /// never be used in any of the following user-generated insertion.
    #[inline(always)]
    pub const fn zero() -> Self {
        Self(Uuid::nil())
    }
}

impl Clone for Replica {
    #[inline(always)]
    fn clone(&self) -> Self {
        let mut lamport_clock = self.lamport_clock;

        lamport_clock.next();

        Self {
            id: ReplicaId::new(),
            insertion_runs: self.insertion_runs.clone(),
            character_ts: 0,
            // run_indexes: self.run_indexes.clone(),
            lamport_clock,
            pending: self.pending.clone(),
        }
    }
}

impl Replica {
    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        self.insertion_runs.assert_invariants()
    }

    #[doc(hidden)]
    pub fn debug(&self) -> debug::Debug<'_> {
        debug::Debug(self)
    }

    #[doc(hidden)]
    pub fn debug_as_btree(&self) -> debug::DebugAsBtree<'_> {
        debug::DebugAsBtree(self)
    }

    /// TODO: docs
    #[inline]
    pub fn deleted<R>(&mut self, range: R) -> CrdtEdit
    where
        R: RangeBounds<usize>,
    {
        #[inline(always)]
        fn range_bounds_to_start_end<R>(
            range: R,
            lo: usize,
            hi: usize,
        ) -> (usize, usize)
        where
            R: RangeBounds<usize>,
        {
            use core::ops::Bound;

            let start = match range.start_bound() {
                Bound::Included(&n) => n,
                Bound::Excluded(&n) => n + 1,
                Bound::Unbounded => lo,
            };

            let end = match range.end_bound() {
                Bound::Included(&n) => n + 1,
                Bound::Excluded(&n) => n,
                Bound::Unbounded => hi,
            };

            (start, end)
        }

        let (start, end) = range_bounds_to_start_end(range, 0, self.len());

        if start == end {
            return CrdtEdit::no_op();
        }

        let start = start as u64;

        let end = end as u64;

        let delete_from = InsertionRun::delete_from;

        let delete_up_to = InsertionRun::delete_up_to;

        let delete_range = InsertionRun::delete_range;

        let (_, _) = self.insertion_runs.delete_range(
            start..end,
            delete_range,
            delete_from,
            delete_up_to,
        );

        //// if it lands within a single fragment we return (deleted_fragment,
        //// split_fragment)
        ////
        //// if it lands on 2 separate fragments we return deleted_fragment,
        //// split_fragment
        ////a
        //if single {

        //self.ids.get_mut(replica_id).split_double(start..end, id_deleted, id_split);
        //} else {
        //    self.ids.get_mut(replica_start).split_run(start, id_deleted);
        //    self.ids.get_mut(replica_end).split_run(end, id_split);
        //}

        //// but whatever the case may be we never need the id's that we land on.

        //match

        CrdtEdit::no_op()
    }

    /// TODO: docs
    #[inline]
    pub fn new(len: u64) -> Self {
        let replica_id = ReplicaId::new();

        let mut lamport_clock = LamportClock::new();

        let origin_run = InsertionRun::new(
            Anchor::origin(),
            replica_id,
            0..len,
            lamport_clock.next(),
        );

        // let run_pointers =
        //     RunIdRegistry::new(insertion_id, origin_run.run_id.clone(), len);

        let insertion_runs = Gtree::new(origin_run);

        Self {
            id: replica_id,
            insertion_runs,
            // run_indexes: run_pointers,
            character_ts: len,
            lamport_clock,
            pending: VecDeque::new(),
        }
    }

    /// TODO: docs
    #[inline]
    pub fn inserted(&mut self, offset: usize, len: usize) -> CrdtEdit {
        if len == 0 {
            return CrdtEdit::no_op();
        }

        let len = len as u64;

        let mut edit = CrdtEdit::no_op();

        let mut inserted_at_id = self.id;
        let mut inserted_at_offset = 0;

        let insert_with = |run: &mut InsertionRun, offset: u64| {
            inserted_at_id = run.replica_id();
            inserted_at_offset = run.start() + offset;

            if offset == run.len()
                && self.id == run.replica_id()
                && self.character_ts == run.end()
            {
                edit = CrdtEdit::insertion(
                    Anchor::new(run.replica_id(), run.end()),
                    self.id,
                    len,
                    run.lamport_ts(),
                );

                run.extend(len);

                return (None, None);
            }

            let range = self.character_ts..self.character_ts + len;

            let lamport_ts = self.lamport_clock.next();

            if offset == 0 {
                let new_run = InsertionRun::new(
                    Anchor::origin(),
                    self.id,
                    range,
                    lamport_ts,
                );

                let run = core::mem::replace(run, new_run);

                edit = CrdtEdit::insertion(
                    Anchor::origin(),
                    self.id,
                    len,
                    lamport_ts,
                );

                (Some(run), None)
            } else {
                let split = run.split(offset);

                let anchor = Anchor::new(run.replica_id(), run.end());

                edit = CrdtEdit::insertion(
                    anchor.clone(),
                    self.id,
                    len,
                    lamport_ts,
                );

                let new_run =
                    InsertionRun::new(anchor, self.id, range, lamport_ts);

                (Some(new_run), split)
            }
        };

        let (inserted_run, split_run) =
            self.insertion_runs.insert(offset as u64, insert_with);

        match (inserted_run, split_run) {
            (Some(inserted_run), Some(split_run)) => {
                //self.ids
                //    .get_mut(inserted_at_id)
                //    .split_run(inserted_at_offset, split_run);

                //self.ids.get_mut(self.id).append_run(len, inserted_run);
            },

            (Some(inserted_run), None) => {
                //self.ids.get_mut(self.id).append_run(len, inserted_run);
            },

            _ => {
                //self.ids.get_mut(self.id).extend_last_run(len),
            },
        }

        self.character_ts += len;

        edit
    }

    /// TODO: docs
    #[allow(clippy::len_without_is_empty)]
    #[doc(hidden)]
    pub fn len(&self) -> usize {
        self.insertion_runs.summary() as _
    }

    /// TODO: docs
    #[inline]
    pub fn merge(&mut self, crdt_edit: CrdtEdit) -> Option<TextEdit> {
        //match crdt_edit {
        //    Cow::Owned(CrdtEdit {
        //        kind:
        //            CrdtEditKind::Insertion {
        //                content,
        //                id,
        //                anchor,
        //                lamport_ts,
        //                len,
        //            },
        //    }) => self.merge_insertion(
        //        Cow::Owned(content),
        //        id,
        //        anchor,
        //        lamport_ts,
        //    ),

        //    Cow::Borrowed(CrdtEdit {
        //        kind:
        //            CrdtEditKind::Insertion {
        //                content,
        //                id,
        //                anchor,
        //                lamport_ts,
        //                len,
        //            },
        //    }) => self.merge_insertion(
        //        Cow::Borrowed(content.as_str()),
        //        id.clone(),
        //        anchor.clone(),
        //        *lamport_ts,
        //    ),

        //    _ => None,
        //}

        todo!()
    }

    fn merge_insertion(
        &mut self,
        anchor: Anchor,
        lamport_ts: LamportTimestamp,
    ) -> Option<TextEdit> {
        todo!();

        //let Some(run_id) = self.run_indexes.get_run_id(&anchor) else {
        //    let crdt_edit = CrdtEdit::insertion(
        //        content.into_owned(),
        //        id,
        //        anchor,
        //        lamport_ts,
        //    );
        //    self.pending.push_back(crdt_edit);
        //    return None;
        //};

        //let len = M::len(&content);

        //let lamport_ts = self.lamport_clock.update(lamport_ts);

        //let offset = downstream::insert(
        //    &mut self.insertion_runs,
        //    &mut self.run_indexes,
        //    id,
        //    lamport_ts,
        //    anchor,
        //    run_id,
        //    len,
        //);

        //Some(TextEdit::new(content, offset..offset))
    }

    /// TODO: docs
    #[inline]
    pub fn replaced<R, T>(&mut self, _byte_range: R, _text: T) -> CrdtEdit
    where
        R: RangeBounds<usize>,
        T: Into<String>,
    {
        todo!();
    }

    /// TODO: docs
    #[inline]
    pub fn undo(&self, _crdt_edit: &CrdtEdit) -> CrdtEdit {
        todo!();
    }
}

mod debug {
    use super::*;

    pub struct Debug<'a>(pub &'a Replica);

    impl<'a> core::fmt::Debug for Debug<'a> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("Replica")
                .field("id", &self.0.id)
                .field("edit_runs", &self.0.insertion_runs)
                // .field("id_registry", &self.0.run_indexes)
                .field("character", &self.0.character_ts)
                .field("lamport", &self.0.lamport_clock)
                .field("pending", &self.0.pending)
                .finish()
        }
    }

    pub struct DebugAsBtree<'a>(pub &'a Replica);

    impl<'a> core::fmt::Debug for DebugAsBtree<'a> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("Replica")
                .field("id", &self.0.id)
                .field("edit_runs", &self.0.insertion_runs.debug_as_btree())
                // .field("id_registry", &self.0.run_indexes)
                .field("character", &self.0.character_ts)
                .field("lamport", &self.0.lamport_clock)
                .field("pending", &self.0.pending)
                .finish()
        }
    }
}
