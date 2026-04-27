//! Provides interface for controlling the terminal

use std::io;

use crate::{buffer::Cell, terminal::Config};

use helix_view::{
    graphics::{CursorKind, Rect},
    theme::Color,
};

#[cfg(all(feature = "termina", not(windows)))]
mod termina;
#[cfg(all(feature = "termina", not(windows)))]
pub use self::termina::TerminaBackend;

#[cfg(all(feature = "termina", windows))]
mod crossterm;
#[cfg(all(feature = "termina", windows))]
pub use self::crossterm::CrosstermBackend;

mod test;
pub use self::test::TestBackend;

/// Representation of a terminal backend.
pub trait Backend {
    /// Claims the terminal for TUI use.
    fn claim(&mut self) -> Result<(), io::Error>;
    /// Update terminal configuration.
    fn reconfigure(&mut self, config: Config) -> Result<(), io::Error>;
    /// Restores the terminal to a normal state, undoes `claim`
    fn restore(&mut self) -> Result<(), io::Error>;
    /// Draws styled text to the terminal
    fn draw<'a, I>(&mut self, content: I) -> Result<(), io::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>;
    /// Draws raw bytes to the terminal (for inline images, graphics protocols, etc.)
    /// Format: (id, x, y, bytes) - id is used for diffing at the Terminal level.
    /// Bytes are `Arc`-shared with `RawContent::payload` to avoid
    /// per-frame copies. Default implementation does nothing.
    fn draw_raw(&mut self, _content: &[crate::buffer::RawWrite]) -> Result<(), io::Error> {
        Ok(())
    }
    /// Deletes images that have scrolled out of viewport.
    /// Default implementation does nothing - backends override if they support graphics.
    fn delete_images(&mut self, _ids: &[u64]) -> Result<(), io::Error> {
        Ok(())
    }
    /// Clears all transmitted images from the screen. Called before each frame redraw.
    /// Default implementation does nothing - backends override if they support graphics.
    fn clear_all_images(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
    /// Syncs image state: deletes any transmitted images not in the current set.
    /// Returns IDs of images that were deleted.
    /// Default implementation does nothing - backends override if they support graphics.
    fn sync_images(&mut self, _current_ids: &[u64]) -> Result<Vec<u64>, io::Error> {
        Ok(Vec::new())
    }
    /// Hides the cursor
    fn hide_cursor(&mut self) -> Result<(), io::Error>;
    /// Sets the cursor to the given shape
    fn show_cursor(&mut self, kind: CursorKind) -> Result<(), io::Error>;
    /// Sets the cursor to the given position
    fn set_cursor(&mut self, x: u16, y: u16) -> Result<(), io::Error>;
    /// Clears the terminal
    fn clear(&mut self) -> Result<(), io::Error>;
    /// Gets the size of the terminal in cells
    fn size(&self) -> Result<Rect, io::Error>;
    /// Begins a synchronized output frame.  Terminals that support
    /// synchronized output will batch all writes until `end_frame` is called.
    fn begin_frame(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
    /// Ends a synchronized output frame, causing the terminal to present.
    fn end_frame(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
    /// Flushes the terminal buffer
    fn flush(&mut self) -> Result<(), io::Error>;
    fn supports_true_color(&self) -> bool;
    fn get_theme_mode(&self) -> Option<helix_view::theme::Mode>;
    fn set_background_color(&mut self, color: Option<Color>) -> io::Result<()>;
}
