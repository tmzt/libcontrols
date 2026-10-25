---
title: Cursor and insertion point in Edit
date: 2026-10-25
status: Implemented
covers:
  - EditState cursor tracking and UTF-8-safe cursor manipulation
  - Edit drawing of SDF vertical cursor bar when active or focused
  - Mouse click and drag hit-testing to nearest character boundary
  - Keyboard navigation (Left, Right, Home, End) and editing (Backspace, Delete, character input)
sources:
  - docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md
  - docs/design/20261023_02_SCROLL_PANE_PLAN.md
related:
  - src/edit.rs
  - src/event.rs
  - src/traits.rs
---

# Cursor and insertion point in Edit

## 1. Motivation and Context

`libcontrols` provides `Edit` and `EditState` in `src/edit.rs` as a single-line
edit box / value readout control. Prior to this plan, `EditState` maintained
only `pub text: Mutex<String>` and `pub is_active: AtomicBool`. `Edit` drew the
text and an active glow ring, but lacked an insertion cursor, had no way to
position an edit point, could not hit-test mouse clicks against text character
positions, and did not handle keyboard navigation (Left, Right, Home, End) or
editing operations (Backspace, Delete, character entry).

This plan adds full cursor / insertion point support:
1. `EditState` tracks the insertion cursor position in bytes (`cursor: Mutex<usize>`),
   always clamped to valid UTF-8 character boundaries within `0..=text.len()`.
2. Safe editing methods on `EditState`: `cursor`, `set_cursor`, `move_cursor_left`,
   `move_cursor_right`, `move_cursor_home`, `move_cursor_end`, `insert_char`,
   `insert_text`, `delete_backwards`, and `delete_forwards`.
3. `Edit` rendering (`DrawControl::draw`): when active or focused, calculates the
   horizontal advance of text before the cursor (`ctx.text_advance(&text[..cursor], font_size)`)
   and draws a vertical SDF cursor bar (`SdfKind::Box`, width 1.5 px, height `line_box_h`,
   color `ctx.theme.accent_color`).
4. Mouse interaction (`HostedControl::handle_mouse`): clicking or dragging in the
   edit box hit-tests the pointer's horizontal coordinate against character boundaries
   and sets the insertion cursor to the nearest boundary.
5. Keyboard interaction (`HostedControl::handle_keyboard`): Left/Right arrows move the
   cursor, Home/End jump to the start/end, Backspace deletes backwards, Delete deletes
   forwards, and character/text entry inserts at the cursor position.

## 2. Interface and Data Layout

### 2.1 `EditState`

```rust
pub struct EditState {
    /// Text content of the edit box.
    pub text: Mutex<String>,
    /// Byte index of insertion cursor within `text` (guaranteed UTF-8 char boundary).
    pub cursor: Mutex<usize>,
    /// Whether the edit box is currently focused/active.
    pub is_active: AtomicBool,
    /// Cached layout metrics from the last draw pass for click hit-testing.
    cached_layout: Mutex<Option<EditLayout>>,
}

#[derive(Clone, Debug)]
struct EditLayout {
    text: String,
    pen_x: f32,
    /// (byte_offset, advance_from_pen_x) for each character boundary from 0 to text.len()
    offsets: Vec<(usize, f32)>,
}
```

#### UTF-8 Safety and Boundary Clamping

A private helper `clamp_to_char_boundary(text: &str, pos: usize) -> usize` ensures
that any cursor position is within `0..=text.len()` and lands on a valid UTF-8 code
point boundary (`text.is_char_boundary(pos)`):

```rust
fn clamp_to_char_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(pos) {
        return pos;
    }
    let mut floor = pos;
    while floor > 0 && !text.is_char_boundary(floor) {
        floor -= 1;
    }
    let mut ceil = pos;
    while ceil < text.len() && !text.is_char_boundary(ceil) {
        ceil += 1;
    }
    if pos - floor <= ceil - pos {
        floor
    } else {
        ceil
    }
}
```

#### New Methods on `EditState`

- `pub fn cursor(&self) -> usize`: reads the cursor byte offset, clamped.
- `pub fn set_cursor(&self, pos: usize)`: sets cursor position, clamped to UTF-8 boundary.
- `pub fn move_cursor_left(&self)`: steps cursor backwards by one character (previous UTF-8 boundary).
- `pub fn move_cursor_right(&self)`: steps cursor forwards by one character (next UTF-8 boundary).
- `pub fn move_cursor_home(&self)`: sets cursor to 0.
- `pub fn move_cursor_end(&self)`: sets cursor to `text.len()`.
- `pub fn insert_char(&self, ch: char)`: inserts `ch` at the cursor and advances cursor by `ch.len_utf8()`.
- `pub fn insert_text(&self, text: &str)`: inserts `text` slice at the cursor and advances cursor by `text.len()`.
- `pub fn delete_backwards(&self) -> bool`: removes the character preceding the cursor and steps cursor back. Returns `true` if a character was deleted.
- `pub fn delete_forwards(&self) -> bool`: removes the character following the cursor. Returns `true` if a character was deleted.
- `pub fn set_text(&self, text: impl Into<String>)`: updates text and clamps cursor to the new text bounds.

### 2.2 `Edit` Control Methods and Drawing

- `pub fn cursor(&self) -> usize`: delegates to `self.state.cursor()`.
- `pub fn set_cursor(&self, pos: usize)`: delegates to `self.state.set_cursor(pos)`.
- `pub fn with_cursor(self, pos: usize) -> Self`: builder helper.
- `pub fn char_index_at_x(&self, click_x: f32) -> usize`: computes character boundary nearest to `click_x` using cached layout or fallback monospace estimation.
- `pub fn set_cursor_from_x(&self, click_x: f32)`: calls `char_index_at_x` and sets the cursor.

#### Visual Cursor Bar

In `DrawControl::draw`:
```rust
if self.is_active() || ctx.state.contains(NodeState::FOCUSED) {
    let cursor_pos = self.state.cursor();
    let safe_cursor = clamp_to_char_boundary(&text, cursor_pos);
    let cursor_offset = ctx.text_advance(&text[..safe_cursor], font_size);
    let cursor_x = pen_x + cursor_offset;
    let cursor_w = 1.5;
    list.push(SdfInstance {
        kind: SdfKind::Box,
        position: [cursor_x, pen_y],
        size: [cursor_w, line_box_h],
        color: ctx.theme.accent_color,
        anim: 0,
    });
    let style = self.glow_style.unwrap_or_default();
    draw_glowing_ring(list, self.rect, &style, &ctx.theme);
}
```

### 2.3 Mouse and Keyboard Handling

- In `handle_mouse`: `MouseEvent::Click` or `MouseEvent::Down` with `MouseButton::Left` computes `let idx = self.char_index_at_x(pos[0])` and calls `self.state.set_cursor(idx)`. `MouseEvent::Drag` with left button similarly updates cursor.
- In `handle_keyboard`:
  - `Key::Left`: moves cursor left, returns `Some(ControlAction::StepLeft)`.
  - `Key::Right`: moves cursor right, returns `Some(ControlAction::StepRight)`.
  - `Key::Home`: moves cursor home, returns `Some(ControlAction::StepLeft)`.
  - `Key::End`: moves cursor end, returns `Some(ControlAction::StepRight)`.
  - `Key::Backspace`: calls `delete_backwards()`, returns `Some(ControlAction::Custom("backspace".into()))`.
  - `Key::Delete`: calls `delete_forwards()`, returns `Some(ControlAction::Custom("delete".into()))`.
  - `Key::Enter`: returns `Some(ControlAction::Activated { part: ctx.part })`.
  - `Key::Space`: calls `insert_char(' ')`, returns `Some(ControlAction::TextInput { ch: ' ' })`.
  - `Key::Char(ch)` / `KeyboardEvent::Char { ch }`: if `!ch.is_control()`, calls `insert_char(ch)` and returns `Some(ControlAction::TextInput { ch })`.

## 3. Implementation Steps

1. **Step 1: `EditState` cursor field, clamping, and editing primitives.**
   Add `pub cursor: Mutex<usize>` and `cached_layout: Mutex<Option<EditLayout>>` to `EditState`. Implement UTF-8 safe cursor clamping, movement (`left`, `right`, `home`, `end`), insertion (`insert_char`, `insert_text`), and deletion (`delete_backwards`, `delete_forwards`).
   **Done when:** unit tests verify UTF-8 boundary safety, bounds clamping `0..=len`, movement, insertion, and deletion on both ASCII and multibyte characters.

2. **Step 2: `Edit` rendering of the cursor bar.**
   In `Edit::draw`, cache character layout metrics in `EditState`, calculate text advance up to cursor position, and push an `SdfKind::Box` instance of width 1.5 px and height `line_box_h` in `ctx.theme.accent_color` when focused or active.
   **Done when:** tests verify that when active/focused, the draw list contains the vertical cursor bar SDF instance at the correct horizontal position.

3. **Step 3: Mouse hit-testing to nearest character boundary.**
   Implement `Edit::char_index_at_x` using cached layout or fallback calculation. In `HostedControl::handle_mouse`, set insertion cursor on Left Click, Down, and Drag.
   **Done when:** tests verify clicking at start, end, intermediate characters, and out-of-bounds positions sets the cursor to the nearest character boundary.

4. **Step 4: Keyboard navigation and text editing.**
   Implement `HostedControl::handle_keyboard` for Left, Right, Home, End, Backspace, Delete, Space, and printable character input.
   **Done when:** tests verify arrow key navigation, Home/End jumping, Backspace/Delete editing, and character insertion through simulated keyboard events.

5. **Step 5: Release and versioning.**
   Because `EditState` gains a public field (`pub cursor: Mutex<usize>`), bump minor version in `Cargo.toml` to `0.9.0` per Critical Rule 5.4. Record API changes in `VERSIONS.md`, update decision table in `README.md`, run formatter, clippy, and full test suite, and tag `v0.9.0`.
   **Done when:** `cargo fmt -- --check`, `cargo clippy --all-targets` with no warnings, and `cargo test` pass cleanly; commit tagged `v0.9.0`.

## Amendment — 2026-10-25 18:58 EDT: implementation started

Implementation has started. This document is now immutable.

## Amendment — 2026-10-25 19:02 EDT: implemented

Every step is done and its **Done when:** holds. Implementation commits:
- Steps 1-4: `5bd956c`
- Step 5 (release 0.9.0): this commit
