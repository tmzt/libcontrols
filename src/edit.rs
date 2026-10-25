//! Single-line edit box and value readout control.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{
    ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseButton, MouseEvent,
};
use crate::flags::NodeState;
use crate::handle::{ControlHandle, ReadValue};
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Helper to clamp a byte index to valid UTF-8 character boundaries within `0..=text.len()`.
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

/// Cached character layout metrics recorded during the last draw pass.
#[derive(Clone, Debug)]
struct EditLayout {
    text: String,
    pen_x: f32,
    offsets: Vec<(usize, f32)>,
}

fn pick_nearest_char_index(click_x: f32, pen_x: f32, offsets: &[(usize, f32)]) -> usize {
    if offsets.is_empty() {
        return 0;
    }
    let rel_x = click_x - pen_x;
    if rel_x <= 0.0 {
        return offsets[0].0;
    }
    let last = offsets[offsets.len() - 1];
    if rel_x >= last.1 {
        return last.0;
    }

    for window in offsets.windows(2) {
        let (idx_left, adv_left) = window[0];
        let (idx_right, adv_right) = window[1];
        if rel_x >= adv_left && rel_x <= adv_right {
            let mid = (adv_left + adv_right) * 0.5;
            return if rel_x < mid { idx_left } else { idx_right };
        }
    }

    last.0
}

/// Mutable runtime state for an edit box / value readout control.
#[derive(Debug)]
pub struct EditState {
    /// Text content of the edit box.
    pub text: Mutex<String>,
    /// Byte index of the insertion cursor within `text` (guaranteed UTF-8 char boundary).
    pub cursor: Mutex<usize>,
    /// Whether the edit box is currently focused/active.
    pub is_active: AtomicBool,
    /// Cached layout metrics from the last draw pass for click hit-testing.
    cached_layout: Mutex<Option<EditLayout>>,
}

impl EditState {
    /// Create a new edit state with initial text content and cursor at the end.
    pub fn new(text: impl Into<String>) -> Self {
        let initial: String = text.into();
        let len = initial.len();
        Self {
            text: Mutex::new(initial),
            cursor: Mutex::new(len),
            is_active: AtomicBool::new(false),
            cached_layout: Mutex::new(None),
        }
    }

    /// Read the current text.
    #[inline]
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Update the text content.
    pub fn set_text(&self, text: impl Into<String>) {
        let new_text = text.into();
        let mut text_guard = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        *cursor = clamp_to_char_boundary(&new_text, *cursor);
        *text_guard = new_text;
        *self.cached_layout.lock().unwrap() = None;
    }

    /// Read the current cursor byte offset.
    #[inline]
    pub fn cursor(&self) -> usize {
        let text = self.text.lock().unwrap();
        let cursor = self.cursor.lock().unwrap();
        clamp_to_char_boundary(&text, *cursor)
    }

    /// Set the cursor byte position, clamped to a valid UTF-8 character boundary.
    pub fn set_cursor(&self, pos: usize) {
        let text = self.text.lock().unwrap();
        *self.cursor.lock().unwrap() = clamp_to_char_boundary(&text, pos);
    }

    /// Move the cursor left by one Unicode scalar value.
    pub fn move_cursor_left(&self) {
        let text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        if cur > 0 {
            *cursor = text[..cur]
                .chars()
                .next_back()
                .map(|c| cur - c.len_utf8())
                .unwrap_or(0);
        } else {
            *cursor = 0;
        }
    }

    /// Move the cursor right by one Unicode scalar value.
    pub fn move_cursor_right(&self) {
        let text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        if cur < text.len() {
            *cursor = text[cur..]
                .chars()
                .next()
                .map(|c| cur + c.len_utf8())
                .unwrap_or(text.len());
        } else {
            *cursor = text.len();
        }
    }

    /// Move the cursor to the beginning of the text (byte offset 0).
    #[inline]
    pub fn move_cursor_home(&self) {
        *self.cursor.lock().unwrap() = 0;
    }

    /// Move the cursor to the end of the text.
    #[inline]
    pub fn move_cursor_end(&self) {
        let text = self.text.lock().unwrap();
        *self.cursor.lock().unwrap() = text.len();
    }

    /// Insert a single character at the current cursor and advance the cursor.
    pub fn insert_char(&self, ch: char) {
        let mut text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        text.insert(cur, ch);
        *cursor = cur + ch.len_utf8();
        *self.cached_layout.lock().unwrap() = None;
    }

    /// Insert a text slice at the current cursor and advance the cursor.
    pub fn insert_text(&self, text_to_insert: &str) {
        let mut text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        text.insert_str(cur, text_to_insert);
        *cursor = cur + text_to_insert.len();
        *self.cached_layout.lock().unwrap() = None;
    }

    /// Delete the character immediately preceding the cursor.
    ///
    /// Returns `true` if a character was deleted, or `false` if the cursor was already at 0.
    pub fn delete_backwards(&self) -> bool {
        let mut text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        if cur == 0 {
            *cursor = 0;
            return false;
        }
        let prev = text[..cur]
            .chars()
            .next_back()
            .map(|c| cur - c.len_utf8())
            .unwrap_or(0);
        text.drain(prev..cur);
        *cursor = prev;
        *self.cached_layout.lock().unwrap() = None;
        true
    }

    /// Delete the character immediately following the cursor.
    ///
    /// Returns `true` if a character was deleted, or `false` if the cursor was at the end of the text.
    pub fn delete_forwards(&self) -> bool {
        let mut text = self.text.lock().unwrap();
        let mut cursor = self.cursor.lock().unwrap();
        let cur = clamp_to_char_boundary(&text, *cursor);
        if cur >= text.len() {
            *cursor = text.len();
            return false;
        }
        let next = text[cur..]
            .chars()
            .next()
            .map(|c| cur + c.len_utf8())
            .unwrap_or(text.len());
        if next == cur {
            return false;
        }
        text.drain(cur..next);
        *cursor = cur;
        *self.cached_layout.lock().unwrap() = None;
        true
    }

    /// Check if the edit control is active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set the active focus state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }
}

/// Single-line edit box / value readout control.
#[derive(Clone, Debug)]
pub struct Edit {
    /// Shared mutable edit state.
    pub state: Arc<EditState>,
    /// Bounding rectangle for the control.
    pub rect: Rect,
    /// Text horizontal alignment within the box.
    pub align: TextAlign,
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
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl Edit {
    /// Create a new edit box with initial text and bounding rectangle.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(EditState::new(text)),
            rect,
            align: TextAlign::Center,
            font_size: None,
            text_color: None,
            bg_color: Some([0.10, 0.13, 0.18, 0.85]),
            border_color: Some([0.15, 0.20, 0.28, 0.60]),
            border_thickness: 1.0,
            corner_radius: None,
            glow_style: None,
        }
    }

    /// Attach an existing shared `EditState`.
    pub fn with_state(mut self, state: Arc<EditState>) -> Self {
        self.state = state;
        self
    }

    /// Read the current text.
    #[inline]
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// Set the text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        self.state.set_text(text);
    }

    /// Read the current cursor byte offset.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.state.cursor()
    }

    /// Set the cursor byte position, clamped to a valid UTF-8 character boundary.
    #[inline]
    pub fn set_cursor(&self, pos: usize) {
        self.state.set_cursor(pos);
    }

    /// Builder helper to set the cursor position.
    pub fn with_cursor(self, pos: usize) -> Self {
        self.state.set_cursor(pos);
        self
    }

    /// Check if active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Set text horizontal alignment.
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set font size.
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Set text color.
    pub fn with_text_color(mut self, color: [f32; 4]) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set background fill color.
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set border color and thickness.
    pub fn with_border(mut self, border_color: [f32; 4], thickness: f32) -> Self {
        self.border_color = Some(border_color);
        self.border_thickness = thickness;
        self
    }

    /// Set corner radius.
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Set active focus state.
    pub fn with_active(self, is_active: bool) -> Self {
        self.state.set_active(is_active);
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Calculate the character byte offset nearest to a horizontal click position.
    pub fn char_index_at_x(&self, click_x: f32) -> usize {
        let text = self.state.text();
        if text.is_empty() {
            return 0;
        }

        let guard = self.state.cached_layout.lock().unwrap();
        if let Some(layout) = guard.as_ref()
            && layout.text == text
            && !layout.offsets.is_empty()
        {
            return pick_nearest_char_index(click_x, layout.pen_x, &layout.offsets);
        }
        drop(guard);

        let font_size = self.font_size.unwrap_or(14.0);
        let char_w = font_size * 0.55;
        let pad_h = 4.0f32;
        let total_advance = text.chars().count() as f32 * char_w;
        let pen_x = match self.align {
            TextAlign::Left => self.rect.x + pad_h,
            TextAlign::Center => self.rect.x + (self.rect.w - total_advance) * 0.5,
            TextAlign::Right => self.rect.x + self.rect.w - pad_h - total_advance,
        };

        let mut offsets = Vec::with_capacity(text.len() + 1);
        offsets.push((0, 0.0));
        let mut char_count = 0;
        for (idx, ch) in text.char_indices() {
            char_count += 1;
            offsets.push((idx + ch.len_utf8(), char_count as f32 * char_w));
        }

        pick_nearest_char_index(click_x, pen_x, &offsets)
    }

    /// Set the cursor to the character boundary nearest to the given X coordinate.
    pub fn set_cursor_from_x(&self, click_x: f32) {
        let idx = self.char_index_at_x(click_x);
        self.state.set_cursor(idx);
    }

    /// Obtain a borrowed handle to inspect the edit box value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

impl DrawControl for Edit {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);
        let text = self.text();

        if let Some(bg_col) = self.bg_color {
            let kind = if rad > 0.0 {
                SdfKind::RoundedBox { radius: rad }
            } else {
                SdfKind::Box
            };
            list.push(SdfInstance {
                kind,
                position: self.rect.pos(),
                size: self.rect.size(),
                color: bg_col,
                anim: 0,
            });
        }

        if let Some(border_col) = self.border_color
            && self.border_thickness > 0.0
        {
            list.push(SdfInstance {
                kind: SdfKind::Outline {
                    radius: rad,
                    thickness: self.border_thickness,
                },
                position: self.rect.pos(),
                size: self.rect.size(),
                color: border_col,
                anim: 0,
            });
        }

        let text_col = self.text_color.unwrap_or(ctx.theme.text_color);
        let text_advance = ctx.text_advance(&text, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pad_h = 4.0f32;

        let pen_x = match self.align {
            TextAlign::Left => self.rect.x + pad_h,
            TextAlign::Center => self.rect.x + (self.rect.w - text_advance) * 0.5,
            TextAlign::Right => self.rect.x + self.rect.w - pad_h - text_advance,
        };
        let pen_y = self.rect.y + (self.rect.h - line_box_h) * 0.5;

        ctx.draw_text(list, &text, [pen_x, pen_y], font_size, text_col);

        let mut offsets = Vec::with_capacity(text.len() + 1);
        offsets.push((0, 0.0));
        for (idx, ch) in text.char_indices() {
            let next_idx = idx + ch.len_utf8();
            let adv = ctx.text_advance(&text[..next_idx], font_size);
            offsets.push((next_idx, adv));
        }
        *self.state.cached_layout.lock().unwrap() = Some(EditLayout {
            text: text.clone(),
            pen_x,
            offsets,
        });

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
    }
}

impl HostedControl for Edit {
    /// A drag on it is its own (text selection/cursor positioning), not a scroll.
    fn claims_drag(&self, _part: ControlPart) -> bool {
        true
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if self.rect.contains(pos) {
            Some((ControlPart::NumericBox, self.rect.normalized_x(pos[0])))
        } else {
            None
        }
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        let hit_part = ctx.part;
        match *event {
            MouseEvent::Click {
                pos,
                button: MouseButton::Left,
            }
            | MouseEvent::Down {
                pos,
                button: MouseButton::Left,
            } => {
                let idx = self.char_index_at_x(pos[0]);
                self.state.set_cursor(idx);
                Some(ControlAction::Activated { part: hit_part })
            }
            MouseEvent::Drag {
                current_pos,
                button: MouseButton::Left,
                ..
            } => {
                let idx = self.char_index_at_x(current_pos[0]);
                self.state.set_cursor(idx);
                None
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        if !event.is_down() {
            return None;
        }

        match event {
            KeyboardEvent::Char { ch } => {
                if !ch.is_control() {
                    self.state.insert_char(*ch);
                    return Some(ControlAction::TextInput { ch: *ch });
                }
            }
            KeyboardEvent::Key { key, .. } | KeyboardEvent::KeyDown { key } => match key {
                Key::Left => {
                    self.state.move_cursor_left();
                    return Some(ControlAction::StepLeft);
                }
                Key::Right => {
                    self.state.move_cursor_right();
                    return Some(ControlAction::StepRight);
                }
                Key::Home => {
                    self.state.move_cursor_home();
                    return Some(ControlAction::StepLeft);
                }
                Key::End => {
                    self.state.move_cursor_end();
                    return Some(ControlAction::StepRight);
                }
                Key::Backspace => {
                    self.state.delete_backwards();
                    return Some(ControlAction::Custom("backspace".into()));
                }
                Key::Delete => {
                    self.state.delete_forwards();
                    return Some(ControlAction::Custom("delete".into()));
                }
                Key::Enter => {
                    return Some(ControlAction::Activated {
                        part: ControlPart::NumericBox,
                    });
                }
                Key::Space => {
                    self.state.insert_char(' ');
                    return Some(ControlAction::TextInput { ch: ' ' });
                }
                Key::Char(ch) => {
                    if !ch.is_control() {
                        self.state.insert_char(*ch);
                        return Some(ControlAction::TextInput { ch: *ch });
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }
}

impl ReadValue<String> for Edit {
    fn read_value(&self) -> String {
        self.text()
    }
}
