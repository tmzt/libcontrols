---
title: TextArea multi-line text edit control
date: 2026-10-25
status: In implementation
covers:
  - TextArea multi-line text input control wrapping text across multiple lines
  - TextAreaState type alias for EditState, reusing UTF-8 cursor positioning and editing primitives
  - Visual styling with padding, font_size, text_color, bg_color, border_color, border_thickness, corner_radius, line_height, glow_style, and placeholder
  - Multi-line SDF text drawing, vertical cursor bar rendering, and active glow ring
  - Mouse click and drag hit-testing to line and character boundaries
  - Keyboard navigation (Up, Down, Left, Right, Home, End) and multi-line editing (Enter, Backspace, Delete, char insertion)
sources:
  - docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md
  - docs/design/20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md
related:
  - src/edit.rs
  - src/text_area.rs
  - src/traits.rs
  - src/control.rs
---

# TextArea multi-line text edit control

## 1. Motivation and Context

`libcontrols` provides single-line text editing via `Edit` and `EditState` (`src/edit.rs`). While `Edit` handles single-line inputs and readouts with UTF-8 cursor tracking, insertion, and deletion, applications requiring multi-line text editing, multi-line prompts, script entry, or notes require a `TextArea` control.

`TextArea` introduces:
1. Multi-line soft text wrapping: lines wrap within available content width (`rect.w - 2.0 * padding[0]`), while also honoring hard line breaks (`\n`).
2. Re-use of `EditState`: text content and insertion cursor tracking are fully compatible with `EditState` (`pub type TextAreaState = EditState;`).
3. Multi-line visual rendering: background box or rounded box, border outline, individual text line rendering with line height spacing, vertical SDF cursor bar (`SdfKind::Box`, width 1.5, height `line_box_h = font_size * LINE_BOX_RATIO`) positioned on the appropriate line and horizontal offset, and an active glow ring. Placeholder text is rendered when the text buffer is empty.
4. Mouse click and drag interaction: vertical coordinate determines the target line; horizontal coordinate hit-tests against character advance boundaries on that line.
5. Keyboard interaction: Left and Right move by character; Up and Down move across lines preserving horizontal column advance where possible; Home and End jump to the line boundaries; Enter inserts a newline character (`\n`); Backspace and Delete perform backward and forward deletion; character and space keys insert text at the cursor.

## 2. Interface and Data Layout

### 2.1 State Alias and Control Definition

```rust
pub type TextAreaState = EditState;

#[derive(Clone, Debug)]
pub struct TextArea {
    /// Shared mutable edit state.
    pub state: Arc<TextAreaState>,
    /// Bounding rectangle for the control.
    pub rect: Rect,
    /// Inner padding [horizontal, vertical].
    pub padding: [f32; 2],
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Optional border outline color.
    pub border_color: Option<[f32; 4]>,
    /// Border thickness in logical pixels.
    pub border_thickness: f32,
    /// Optional corner radius.
    pub corner_radius: Option<f32>,
    /// Optional line height multiplier (defaults to 1.3).
    pub line_height: Option<f32>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
    /// Optional placeholder string displayed when empty.
    pub placeholder: Option<String>,
}
```

### 2.2 Text Wrapping and Line Metrics

Given text content, available width `max_width = (rect.w - 2.0 * padding[0]).max(0.0)`, and a measurement closure `measure(&str) -> f32`:

1. Text is partitioned by newline (`\n`) characters into paragraphs.
2. If a paragraph fits within `max_width`, it becomes a single `LineLayout`.
3. If a paragraph exceeds `max_width`, it is soft-wrapped by finding the latest whitespace character before `max_width`. If no whitespace exists (e.g. a long word), it breaks mid-word at the maximum character boundary fitting within `max_width` (guaranteeing at least one character per line).
4. Each `LineLayout` records:
   - `start`: byte offset in `text` where the line starts.
   - `end`: byte offset in `text` where the line's visible characters end.
   - `next_start`: byte offset where the subsequent line begins (advancing past `\n` if present).
   - `width`: horizontal advance in logical pixels.

### 2.3 Drawing (`DrawControl::draw`)

1. **Background**: If `bg_color` is present, renders `SdfKind::RoundedBox { radius }` (or `SdfKind::Box` if radius is 0) with size `rect.size()`.
2. **Border**: If `border_color` is present and `border_thickness > 0.0`, renders `SdfKind::Outline` with size `rect.size()`.
3. **Text / Placeholder**:
   - If text is empty and `placeholder` is set, wraps and renders placeholder text using dimmed text color.
   - Otherwise, wraps `text` into lines and draws each line at `[pen_origin_x, pen_origin_y + line_index * line_step]`, where `line_step = font_size * line_height.unwrap_or(1.3)`.
4. **Cursor and Glow**:
   - When active (`self.is_active()`) or focused (`ctx.state.contains(NodeState::FOCUSED)`):
     - Locates the line containing the current cursor byte offset (`self.cursor()`).
     - Computes the horizontal advance from the line's start up to the cursor: `ctx.text_advance(&text[line.start..cursor], font_size)`.
     - Draws a vertical cursor bar: `SdfKind::Box` at `[pen_origin_x + cursor_advance, pen_origin_y + line_index * line_step]`, size `[1.5, font_size * LINE_BOX_RATIO]`, color `ctx.theme.accent_color`.
     - Renders an active glowing ring around `self.rect`.

### 2.4 Mouse Hit-Testing (`HostedControl::handle_mouse`)

- On `MouseEvent::Click`, `MouseEvent::Down`, or `MouseEvent::Drag` with `MouseButton::Left`:
  - Vertical coordinate selects the target line: `((pos[1] - (rect.y + padding[1])) / line_step).floor()`, clamped to valid line indices.
  - Horizontal coordinate hit-tests character boundaries on the target line using `ctx.text_advance` or monospace metrics (`font_size * 0.55`).
  - Sets the cursor in `self.state.set_cursor(target_pos)`.
  - Claims drags (`claims_drag` returns `true`) so parent scroll panes do not intercept cursor positioning.

### 2.5 Keyboard Handling (`HostedControl::handle_keyboard`)

- `Key::Left`: moves cursor left by one code point.
- `Key::Right`: moves cursor right by one code point.
- `Key::Up`: moves cursor to the line above, targeting the same horizontal pixel advance.
- `Key::Down`: moves cursor to the line below, targeting the same horizontal pixel advance.
- `Key::Home`: moves cursor to the beginning of the current line (`line.start`).
- `Key::End`: moves cursor to the end of the current line (`line.end`).
- `Key::Enter`: inserts a newline `'\n'` at the cursor.
- `Key::Backspace`: deletes the character preceding the cursor.
- `Key::Delete`: deletes the character following the cursor.
- `Key::Space` / `Key::Char(ch)` / `KeyboardEvent::Char { ch }`: inserts the character at the cursor.

## 3. Implementation Steps

1. **Step 1: `TextArea` data structure and line wrapping layout.**
   Create `src/text_area.rs` defining `pub type TextAreaState = EditState;` and `pub struct TextArea` with constructors, builder methods, and the line wrapping layout engine.
   **Done when:** unit tests verify that multi-line text with explicit newlines and soft-wrapped long lines produces correct line counts, byte ranges, and cursor-to-(line, col) mapping.

2. **Step 2: SDF rendering of multi-line text, cursor, and glow ring.**
   Implement `DrawControl` for `TextArea`: drawing background, outline border, wrapped text lines, vertical cursor bar at `[cursor_x, cursor_y]`, and active glow ring.
   **Done when:** tests verify that rendering an active or focused `TextArea` emits the appropriate `SdfInstance` items for background, lines, and cursor bar into the `DrawList`.

3. **Step 3: Mouse hit-testing and cursor placement.**
   Implement `HostedControl::hit_test`, `claims_drag`, and `handle_mouse` to map pointer clicks and drags to the nearest line and character boundary.
   **Done when:** tests verify that clicking on various lines and horizontal coordinates updates the cursor to the nearest character boundary.

4. **Step 4: Keyboard navigation and editing.**
   Implement `HostedControl::handle_keyboard` for Up, Down, Left, Right, Home, End, Enter, Backspace, Delete, and character entry.
   **Done when:** tests verify that arrow keys navigate across lines and characters, Enter inserts newlines, Home/End jump to line boundaries, and Backspace/Delete mutate text properly.

5. **Step 5: Release and versioning.**
   Re-export `TextArea` and `TextAreaState` in `src/lib.rs`. Bump patch version in `Cargo.toml` to `0.9.1` (strictly additive per Rule 5.4). Record addition in `VERSIONS.md`, update decision table in `README.md`, run formatter, clippy, and tests, tag `v0.9.1`, and push.
   **Done when:** `cargo fmt -- --check`, `cargo clippy --all-targets` with no warnings, and `cargo test` pass cleanly; commit tagged `v0.9.1` pushed to remote.

## Amendment — 2026-10-25 22:52 EDT: implementation started

Implementation has started. This document is now immutable.
