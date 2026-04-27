//! Terminal graphics protocol abstraction.
//!
//! The TUI emits inline images via terminal-specific escape sequences
//! (Kitty graphics, sixel, iTerm2 inline images, etc.). Each protocol
//! has its own transmit, delete, and identity rules. [`GraphicsProtocol`]
//! is the single contract a backend talks to so the rest of the rendering
//! pipeline ([`crate::terminal::Terminal::flush`], [`crate::buffer::Buffer`])
//! and consumers in `helix-term` (Steel FFI image bindings) stay free of
//! protocol-specific escape literals.
//!
//! Adding a new protocol = add a unit-struct impl in a sibling module.
//! The backend's `delete_images` / `clear_all_images` / `sync_images`
//! routes through the trait; nothing else needs changing.

use std::io;

pub mod kitty;
pub use kitty::KittyProtocol;

/// Contract a terminal graphics protocol must satisfy so the renderer
/// can manage image lifecycle without knowing the wire format.
pub trait GraphicsProtocol {
    /// Emit the protocol's "delete image by id" escape sequence to `w`.
    /// Used by `Backend::delete_images`, `clear_all_images`, and
    /// `sync_images` to retire individual cached images by id.
    fn write_delete(&self, id: u64, w: &mut dyn io::Write) -> io::Result<()>;

    /// Parse a transmission payload to extract the image's identity.
    ///
    /// `RawContent` ids are also used as cache keys by the protocol, so
    /// callers that build a `RawContent` from a payload they did not
    /// generate themselves (e.g. Steel plugins handing the editor a
    /// pre-encoded escape) should run the payload through this so the
    /// editor's id matches the protocol's id 1:1. Returns `None` if the
    /// payload is malformed or the protocol cannot read an id from it.
    fn extract_image_id(&self, payload: &str) -> Option<u64>;
}
