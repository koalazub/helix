//! Off-grid graphics state. Owned by
//! [`crate::terminal::Terminal`], not [`crate::buffer::Buffer`] — the
//! cell grid stays protocol-agnostic, while a [`RawSurface`] is the
//! *parallel* layer holding the bytes the renderer wants to emit
//! between cell paints (Kitty/sixel/iTerm2 image transmissions) plus
//! a queue of explicit per-id deletes.
//!
//! Each frame, [`crate::terminal::Terminal::flush`] consults both —
//! diffing writes against the previous frame's surface to decide
//! which ids need (re)transmission, and forwarding deletes to the
//! [`crate::graphics::GraphicsProtocol`].
//!
//! The id is a protocol-level cache key: for Kitty it is the same
//! `i=` value the transmission escape carries, so the two ends use
//! one name for one thing.

use std::sync::Arc;

/// A single raw byte write addressed by `(id, x, y)`.
///
/// `id` is the image identity (matches the protocol's cache key,
/// which `RawContent::id` mirrors); `(x, y)` is the cell position the
/// renderer wants the bytes anchored at; `bytes` is the protocol's
/// transmission/placement escape, shared by `Arc` so producers,
/// front/back surfaces, and the backend never copy the payload.
pub type RawWrite = (u64, u16, u16, Arc<Vec<u8>>);

/// Per-buffer collection of raw writes and explicit-delete queue.
///
/// Mutations go through [`Self::write`] and [`Self::delete`]; reads
/// expose slices so [`crate::terminal::Terminal::flush`] can diff them
/// against the previous frame's surface without allocation.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RawSurface {
    writes: Vec<RawWrite>,
    /// Image ids the producer has explicitly asked the backend to
    /// drop on the next flush. Distinct from "ids that were in the
    /// previous frame but not this one" — see the long comment in
    /// `Terminal::flush` for why automatic delete-on-disappear is the
    /// wrong policy under Kitty's Unicode placeholder protocol.
    pending_deletes: Vec<u64>,
}

impl RawSurface {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a transmission/placement byte string for `id` at
    /// `(x, y)`. Producers normally pass an `Arc` they already hold
    /// (e.g. `RawContent::payload.clone()` is one ref-count bump).
    /// Multiple writes for the same id in one frame are allowed; the
    /// renderer dedupes by id when it diffs against the previous
    /// frame.
    pub fn write(&mut self, id: u64, x: u16, y: u16, bytes: Arc<Vec<u8>>) {
        self.writes.push((id, x, y, bytes));
    }

    /// Queue an explicit delete for `id`. No-op if `id` is already
    /// queued in the same frame.
    pub fn delete(&mut self, id: u64) {
        if !self.pending_deletes.contains(&id) {
            self.pending_deletes.push(id);
        }
    }

    pub fn writes(&self) -> &[RawWrite] {
        &self.writes
    }

    pub fn pending_deletes(&self) -> &[u64] {
        &self.pending_deletes
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty() && self.pending_deletes.is_empty()
    }

    /// Wipe the surface. Called from [`crate::buffer::Buffer::reset`]
    /// so the next frame starts from a clean slate.
    pub fn clear(&mut self) {
        self.writes.clear();
        self.pending_deletes.clear();
    }

    /// Iterate the protocol image ids referenced by this frame's
    /// writes. Used by [`crate::terminal::Terminal::flush`] to build
    /// the "current ids" set passed to
    /// [`crate::backend::Backend::sync_images`].
    pub fn image_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.writes.iter().map(|(id, _, _, _)| *id)
    }
}
