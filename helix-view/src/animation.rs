//! Animation timing policy for inline-image overlays.
//!
//! [`AnimationConfig`] is the user-facing knob (redraw interval and an
//! informational FPS cap). [`AnimationOrchestrator`] is the policy that
//! decides, per redraw tick, how long the editor should wait before the
//! next frame given whether any document is currently animating.
//!
//! The query — *is anything animating right now?* — lives on
//! [`crate::Editor`] (it needs `&self.documents`). The *policy* —
//! clamp configured interval to a safe floor, fall back to a slower
//! cadence when nothing is animating — lives here so the redraw loop
//! does not interleave magic numbers with editor state.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Idle redraw cadence used when no document has animated overlays.
/// Roughly 30 fps, matching the historic default before animation
/// overlays were introduced.
pub const IDLE_REDRAW_INTERVAL: Duration = Duration::from_millis(33);

/// Lower bound on the configured animation interval. Below this the
/// redraw loop would burn CPU faster than terminals can repaint.
pub const MIN_ANIMATION_INTERVAL: Duration = Duration::from_millis(8);

/// User-facing animation configuration.
///
/// Sits on [`crate::editor::Config`]; defaults match the original
/// inline values that lived in the redraw loop.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AnimationConfig {
    /// Target redraw interval while an animated overlay is on screen,
    /// in milliseconds. Defaults to 16 ms (~60 fps). Values below
    /// [`MIN_ANIMATION_INTERVAL`] are clamped at runtime.
    pub redraw_interval_ms: u64,
    /// Maximum frames-per-second for animated overlays. Informational
    /// only; the effective cap comes from `redraw_interval_ms`.
    pub max_fps: u32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            redraw_interval_ms: 16, // ~60 fps
            max_fps: 60,
        }
    }
}

impl AnimationConfig {
    /// Configured interval as a [`Duration`], clamped to the floor.
    pub fn redraw_interval(&self) -> Duration {
        Duration::from_millis(self.redraw_interval_ms).max(MIN_ANIMATION_INTERVAL)
    }
}

/// Picks the next redraw interval given the current animation state.
///
/// Stateless — borrows a config and answers questions. Construct fresh
/// at each redraw tick rather than caching.
pub struct AnimationOrchestrator<'a> {
    config: &'a AnimationConfig,
}

impl<'a> AnimationOrchestrator<'a> {
    pub fn new(config: &'a AnimationConfig) -> Self {
        Self { config }
    }

    /// Time the redraw loop should sleep before the next frame.
    /// `any_animating` is the snapshot of whether *any* document
    /// currently carries animated raw content (see
    /// [`crate::Editor::any_doc_has_animating_content`]).
    pub fn next_interval(&self, any_animating: bool) -> Duration {
        if any_animating {
            self.config.redraw_interval()
        } else {
            IDLE_REDRAW_INTERVAL
        }
    }
}
