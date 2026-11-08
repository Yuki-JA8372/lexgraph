//! Snapshot-based undo and redo.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A field whose consecutive edits share one undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditField<'a> {
    Source(u64),
    Width(u64),
    IntegralBound(u64, bool),
    CurveRange(u64, bool),
    /// A parameter's name, and which of its values is edited.
    Param(&'a str, u8),
}

impl EditField<'_> {
    pub fn key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    limit: usize,
    /// The field last edited; further edits to it share one undo step.
    coalesce_key: Option<u64>,
}

impl<T> History<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit,
            coalesce_key: None,
        }
    }

    /// Stores the state from before an edit as its own undo step.
    pub fn record(&mut self, before: T) {
        self.coalesce_key = None;
        self.push(before);
    }

    /// Like [`Self::record`], but consecutive edits with the same key make one step.
    pub fn record_coalesced(&mut self, key: u64, before: T) {
        if self.coalesce_key != Some(key) {
            self.coalesce_key = Some(key);
            self.push(before);
        }
    }

    /// The current gesture is over: the next edit starts a new step, even in the same field.
    pub fn end_coalescing(&mut self) {
        self.coalesce_key = None;
    }

    fn push(&mut self, before: T) {
        self.redo.clear();
        self.undo.push(before);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, current: T) -> Option<T> {
        self.coalesce_key = None;
        let previous = self.undo.pop()?;
        self.redo.push(current);
        Some(previous)
    }

    pub fn redo(&mut self, current: T) -> Option<T> {
        self.coalesce_key = None;
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }
}
