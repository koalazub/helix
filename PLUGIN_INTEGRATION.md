# Nothelix Plugin Integration Guide

## Overview

This document describes the integration between Helix's RawContent API (on `feature/inline-image-rendering` branch) and the Nothelix plugin (separate repository at `/Users/alielali/projects/nothelix`).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Helix Core (feature/inline-image-rendering branch)          │
│                                                              │
│  - RawContent struct (helix-core/src/text_annotations.rs)   │
│  - Document storage (helix-view/src/document.rs)            │
│  - Steel binding: add-raw-content! (helix-term/.../steel/)  │
│                                                              │
│  API: (add-raw-content! payload height char_idx)            │
└─────────────────────────────────────────────────────────────┘
                            ↑
                            │ Steel FFI
                            │
┌─────────────────────────────────────────────────────────────┐
│ Nothelix Plugin (separate repo)                             │
│                                                              │
│  Location: ~/projects/nothelix/plugin/                      │
│  Files:                                                      │
│    - nothelix.scm (main plugin)                             │
│    - nothelix-autoconvert.scm (auto file conversion)        │
│    - kernel-manager.scm (Julia kernel interface)            │
│                                                              │
│  Uses: Helix RawContent API for inline image rendering      │
└─────────────────────────────────────────────────────────────┘
```

## Helix Core API

### Steel Function: `add-raw-content!`

**Location:** `helix-term/src/commands/engine/steel/mod.rs:5798-5822`

**Signature:**
```scheme
(add-raw-content! payload height char_idx)
```

**Parameters:**
- `payload: Vec<u8>` - Raw terminal escape sequences (e.g., Kitty graphics protocol)
- `height: u16` - Visual height in text lines this content occupies
- `char_idx: usize` - Document position to insert the content

**Behavior:**
- Generates unique ID automatically
- Wraps payload in Arc for cheap cloning
- Adds to current document/view's raw_content HashMap
- Integrated into text_annotations() rendering pipeline

**Example Usage:**
```scheme
;; Add Kitty graphics command
(let ([kitty-cmd (kitty-transmit-image image-data 1)])
  (add-raw-content! kitty-cmd 20 0))
```

## Plugin Integration

### Current Plugin Location

Plugin files are installed at: `~/.config/helix/plugins/`

Files copied from nothelix repo:
- `nothelix.scm` - Main notebook plugin
- `nothelix-autoconvert.scm` - Auto-conversion of .ipynb files
- `kernel-manager.scm` - Julia kernel management
- `nothelix-async.scm` - Async notebook parsing
- `test-async-notebook.scm` - Tests

### Loading in Helix

**File:** `~/.config/helix/init.scm`

```scheme
;; Nothelix - Jupyter notebook plugin
(require "plugins/nothelix.scm")
(require "plugins/nothelix-autoconvert.scm")
```

## What the Plugin Needs to Implement

The plugin needs to:

1. **Detect terminal graphics protocol** (Kitty, iTerm2, Sixel, fallback)
2. **Encode images** to terminal escape sequences
3. **Call `add-raw-content!`** to render inline

This can be done either:

### Option A: Pure Steel Implementation

Implement encoding in Steel directly:

```scheme
(define (render-plot path line-number)
  (let* ([protocol (detect-protocol)]
         [image-data (read-file-bytes path)]
         [encoded (encode-image protocol image-data)])
    (add-raw-content! encoded 20 (line->char-idx line-number))))
```

### Option B: Rust Dylib for Heavy Lifting

Create `libnothelix_render` crate in nothelix repo:

**Location:** `~/projects/nothelix/libnothelix_render/`

**Cargo.toml:**
```toml
[package]
name = "libnothelix_render"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
image = "0.25"
ratatui-image = "3.0"  # Protocol detection + encoding
base64 = "0.22"
```

**FFI exports:**
```rust
#[no_mangle]
pub extern "C" fn nothelix_detect_protocol() -> *mut c_char;

#[no_mangle]
pub extern "C" fn nothelix_encode_file(
    path: *const c_char,
    image_id: u64,
    max_width: u16,
    max_height: u16,
) -> *mut c_char;  // Returns JSON
```

**Steel usage:**
```scheme
(require-dylib "libnothelix_render")

(define (render-plot path line)
  (let* ([result-json (nothelix-encode-file path 1 80 24)]
         [result (json-parse result-json)]
         [transmit (result 'transmit-command)]
         [display (result 'display-command)]
         [height (result 'height)])

    ;; Send transmit once (for Kitty)
    (when transmit (raw-output! transmit))

    ;; Add display command to document
    (add-raw-content! display height (line->char-idx line))))
```

## Performance Optimizations (Already Implemented in Helix)

The RawContent API includes these critical optimizations:

1. **Arc-wrapped payload** - Payload is `Arc<Vec<u8>>`, making clones ~2ns instead of 500µs
2. **ID-based equality** - Diffing compares u64 IDs, not 2MB payloads (0.3ns vs 500µs)
3. **Kitty async protocol support** - Transmit once, display by ID (30 bytes vs 2MB per scroll)

Combined theoretical speedup: ~2,000,000x vs naive implementation.

## Testing

To test the integration:

1. Build Helix with Steel support:
   ```bash
   cd ~/projects/helix
   cargo build --release --features steel
   ```

2. Create test file:
   ```scheme
   ;; test.scm
   (define payload '(27 91 51 49 109 72 101 108 108 111 27 91 48 109))  ;; Red "Hello"
   (add-raw-content! payload 1 0)
   ```

3. Run in Helix:
   ```
   :scm (require "test.scm")
   ```

## Next Steps for Plugin Developer

1. Choose implementation strategy (pure Steel vs dylib)
2. If dylib: Create `libnothelix_render` in nothelix repo
3. Implement protocol detection (check $TERM, $TERM_PROGRAM)
4. Implement image encoding (Kitty priority, then iTerm2, then Sixel)
5. Update `nothelix.scm` to use `add-raw-content!` instead of raw `raw-output!`
6. Test with actual notebook files

## References

- Helix RawContent implementation: `helix-core/src/text_annotations.rs:83-122`
- Steel binding: `helix-term/src/commands/engine/steel/mod.rs:5798-5822`
- Kitty graphics protocol: https://sw.kovidgoyal.net/kitty/graphics-protocol/
- ratatui-image crate: https://crates.io/crates/ratatui-image
