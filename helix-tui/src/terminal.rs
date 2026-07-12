//! Terminal interface provided through the [Terminal] type.
//! Frontend for [Backend]

use crate::{
    backend::Backend,
    buffer::{Buffer, RawSurface},
};
use helix_view::editor::{Config as EditorConfig, KittyKeyboardProtocolConfig};
use helix_view::graphics::{CursorKind, Rect};
use std::io;

#[derive(Debug, Clone, PartialEq)]
/// UNSTABLE
enum ResizeBehavior {
    Fixed,
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
/// UNSTABLE
pub struct Viewport {
    area: Rect,
    resize_behavior: ResizeBehavior,
}

/// Terminal configuration
#[derive(Debug)]
pub struct Config {
    pub enable_mouse_capture: bool,
    pub force_enable_extended_underlines: bool,
    pub kitty_keyboard_protocol: KittyKeyboardProtocolConfig,
}

impl From<&EditorConfig> for Config {
    fn from(config: &EditorConfig) -> Self {
        Self {
            enable_mouse_capture: config.mouse,
            force_enable_extended_underlines: config.undercurl,
            kitty_keyboard_protocol: config.kitty_keyboard_protocol,
        }
    }
}

impl Viewport {
    /// UNSTABLE
    pub fn fixed(area: Rect) -> Viewport {
        Viewport {
            area,
            resize_behavior: ResizeBehavior::Fixed,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Options to pass to [`Terminal::with_options`]
pub struct TerminalOptions {
    /// Viewport used to draw to the terminal
    pub viewport: Viewport,
}

/// Interface to the terminal backed by crossterm
#[derive(Debug)]
pub struct Terminal<B>
where
    B: Backend,
{
    backend: B,
    /// Holds the results of the current and previous draw calls. The two are compared at the end
    /// of each draw pass to output the necessary updates to the terminal
    buffers: [Buffer; 2],
    /// Off-grid graphics state, parallel to `buffers` and indexed by
    /// `current` in lockstep. Producers fill the active surface
    /// during a frame; `flush` diffs current against previous to
    /// decide which images need (re)transmission.
    graphics: [RawSurface; 2],
    /// Index of the current buffer in the previous array
    current: usize,
    /// Kind of cursor (hidden or others)
    cursor_kind: CursorKind,
    /// Viewport
    viewport: Viewport,
    /// Set to request a full clear. The erase is deferred to the next `flush` so it is emitted
    /// inside the same synchronized-output frame as the repaint to avoid painting blank frames
    force_clear: bool,
}

/// Default terminal size: 80 columns, 24 lines
pub const DEFAULT_TERMINAL_SIZE: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

impl<B> Terminal<B>
where
    B: Backend,
{
    /// Wrapper around Terminal initialization. Each buffer is initialized with a blank string and
    /// default colors for the foreground and the background
    pub fn new(backend: B) -> io::Result<Terminal<B>> {
        let size = backend.size().unwrap_or(DEFAULT_TERMINAL_SIZE);
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport {
                    area: size,
                    resize_behavior: ResizeBehavior::Auto,
                },
            },
        )
    }

    /// UNSTABLE
    pub fn with_options(backend: B, options: TerminalOptions) -> io::Result<Terminal<B>> {
        Ok(Terminal {
            backend,
            buffers: [
                Buffer::empty(options.viewport.area),
                Buffer::empty(options.viewport.area),
            ],
            graphics: [RawSurface::new(), RawSurface::new()],
            current: 0,
            cursor_kind: CursorKind::Block,
            viewport: options.viewport,
            force_clear: false,
        })
    }

    pub fn claim(&mut self) -> io::Result<()> {
        self.backend.claim()
    }

    pub fn reconfigure(&mut self, config: Config) -> io::Result<()> {
        self.backend.reconfigure(config)
    }

    pub fn restore(&mut self) -> io::Result<()> {
        self.backend.restore()
    }

    // /// Get a Frame object which provides a consistent view into the terminal state for rendering.
    // pub fn get_frame(&mut self) -> Frame<B> {
    //     Frame {
    //         terminal: self,
    //         cursor_position: None,
    //     }
    // }

    /// Combined mutable accessor for the active draw surfaces. Returns
    /// `(cell grid, off-grid graphics)`. Used by the renderer to thread
    /// both into compositor::Context without repeated `&mut self` calls.
    ///
    /// Deliberately the only buffer accessor — exposing a buffer-only
    /// `current_buffer_mut` would let callers paint cells without
    /// touching the parallel graphics surface, and that lockstep is
    /// load-bearing for image diffing.
    pub fn current_buffer_and_raw_mut(&mut self) -> (&mut Buffer, &mut RawSurface) {
        let Self {
            buffers,
            graphics,
            current,
            ..
        } = self;
        (&mut buffers[*current], &mut graphics[*current])
    }

    pub fn current_raw_mut(&mut self) -> &mut RawSurface {
        &mut self.graphics[self.current]
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn flush(&mut self) -> io::Result<()> {
        if self.force_clear {
            self.backend.clear()?;
            self.force_clear = false;
        }
        let previous_buffer = &self.buffers[1 - self.current];
        let current_buffer = &self.buffers[self.current];
        let previous_raw = &self.graphics[1 - self.current];
        let current_raw = &self.graphics[self.current];

        let current_image_ids: Vec<u64> = current_raw.image_ids().collect();

        self.backend.sync_images(&current_image_ids)?;

        let prev_image_ids: std::collections::HashSet<u64> = previous_raw.image_ids().collect();

        // Image delete policy. This editor uses Kitty's Unicode
        // placeholder protocol (`U=1`) for inline plots — the image is
        // transmitted once under a stable id, Kitty caches it, and the
        // placeholder cells written into the normal text grid reference
        // it by id wherever they happen to be drawn. In that world the
        // auto-delete logic an earlier version of this flush used
        // (anything in prev but not in curr → delete) actively breaks
        // things: when the user switches buffers or scrolls the plot
        // out of view, the RawContent stops emitting raw_writes for a
        // frame, the auto-delete fires, Kitty drops the cache entry,
        // and when the user comes back the placeholder cells have no
        // image to resolve to. We'd then re-transmit on the next frame,
        // but the user briefly sees empty space where the plot should
        // be, and under certain race conditions the re-transmission
        // doesn't land.
        //
        // Only honour *explicit* deletes now — anything the plugin
        // has queued on `pending_deletes` by calling
        // `delete_raw_image` or clearing raw content. That's a small
        // behavioural regression for any future direct-placement
        // (`a=T`) consumer that relied on auto-cleanup, but the old
        // path is deprecated and nothelix no longer uses it.
        let to_delete: Vec<u64> = current_raw.pending_deletes().to_vec();

        // Transmit virtual-placement images once on first sighting.
        // Position changes don't require retransmission because the
        // placeholder cells drive rendering, not the transmission's
        // anchor point.
        let to_draw: Vec<&crate::buffer::RawWrite> = current_raw
            .writes()
            .iter()
            .filter(|(id, _, _, _)| !prev_image_ids.contains(id))
            .collect();

        if !to_delete.is_empty() {
            self.backend.delete_images(&to_delete)?;
        }

        // Begin synchronized output frame — all writes between begin_frame()
        // and end_frame() are batched by the terminal and presented atomically.
        // This ensures raw image data (draw_raw) is rendered in the same frame
        // as the cell updates (draw), preventing flicker and missed images.
        self.backend.begin_frame()?;

        let updates = previous_buffer.diff(current_buffer);
        self.backend.draw(updates.into_iter())?;

        if !to_draw.is_empty() {
            let draw_data: Vec<crate::buffer::RawWrite> = to_draw
                .into_iter()
                .map(|(id, x, y, bytes)| (*id, *x, *y, bytes.clone()))
                .collect();
            self.backend.draw_raw(&draw_data)?;
        }

        self.backend.end_frame()?;

        Ok(())
    }

    /// Updates the Terminal so that internal buffers match the requested size. Requested size will
    /// be saved so the size can remain consistent when rendering.
    pub fn resize(&mut self, area: Rect) -> io::Result<()> {
        self.buffers[self.current].resize(area);
        self.buffers[1 - self.current].resize(area);
        self.viewport.area = area;
        self.clear()
    }

    /// Queries the backend for size and resizes if it doesn't match the previous size.
    pub fn autoresize(&mut self) -> io::Result<Rect> {
        let size = self.size();
        if size != self.viewport.area {
            self.resize(size)?;
        };
        Ok(size)
    }

    /// Synchronizes terminal size, calls the rendering closure, flushes the current internal state
    /// and prepares for the next draw call.
    pub fn draw(
        &mut self,
        cursor_position: Option<(u16, u16)>,
        cursor_kind: CursorKind,
    ) -> io::Result<()> {
        // // Autoresize - otherwise we get glitches if shrinking or potential desync between widgets
        // // and the terminal (if growing), which may OOB.
        // self.autoresize()?;

        // let mut frame = self.get_frame();
        // f(&mut frame);
        // // We can't change the cursor position right away because we have to flush the frame to
        // // stdout first. But we also can't keep the frame around, since it holds a &mut to
        // // Terminal. Thus, we're taking the important data out of the Frame and dropping it.
        // let cursor_position = frame.cursor_position;

        // One synchronized frame for the whole draw
        self.backend.start_sync()?;

        // Draw to stdout
        self.flush()?;

        if let Some((x, y)) = cursor_position {
            self.set_cursor(x, y)?;
        }

        match cursor_kind {
            CursorKind::Hidden => self.hide_cursor()?,
            kind => self.show_cursor(kind)?,
        }

        self.backend.end_sync()?;

        // Swap buffers and graphics surfaces in lockstep.
        self.buffers[1 - self.current].reset();
        self.graphics[1 - self.current].clear();
        self.current = 1 - self.current;

        // Flush
        self.backend.flush()?;
        Ok(())
    }

    #[inline]
    pub fn cursor_kind(&self) -> CursorKind {
        self.cursor_kind
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        self.cursor_kind = CursorKind::Hidden;
        Ok(())
    }

    pub fn show_cursor(&mut self, kind: CursorKind) -> io::Result<()> {
        self.backend.show_cursor(kind)?;
        self.cursor_kind = kind;
        Ok(())
    }

    pub fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        self.backend.set_cursor(x, y)
    }

    /// Clear the terminal and force a full redraw on the next draw call.
    ///
    /// The physical erase is deferred to the next `flush` so it shares a
    /// synchronized frame with the repaint.
    pub fn clear(&mut self) -> io::Result<()> {
        self.force_clear = true;
        // Reset the back buffer + graphics so the next update will
        // redraw everything (including retransmitting cached images).
        // The physical erase is deferred to the next `flush` so it shares
        // a synchronized frame with the repaint.
        self.buffers[1 - self.current].reset();
        self.graphics[1 - self.current].clear();
        Ok(())
    }

    /// Queries the real size of the backend.
    pub fn size(&self) -> Rect {
        self.backend.size().unwrap_or(DEFAULT_TERMINAL_SIZE)
    }
}
