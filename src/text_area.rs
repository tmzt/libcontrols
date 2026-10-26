//! Multi-line text edit control with cursor and insertion point support.

use std::sync::Arc;

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::edit::EditState;
use crate::event::{
    ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseButton, MouseEvent,
};
use crate::flags::NodeState;
use crate::handle::{ControlHandle, ReadValue};
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Helper to clamp a byte index to valid UTF-8 character boundaries within `0..=text.len()`.
#[inline]
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

/// Single rendered line layout within the text area.
#[derive(Clone, Debug, PartialEq)]
pub struct LineLayout {
    /// Byte index in text where this line's content begins.
    pub start: usize,
    /// Byte index in text where this line's visible content ends (before \n if any).
    pub end: usize,
    /// Byte index in text where the next line begins (past \n or soft wrap point).
    pub next_start: usize,
    /// Advance width in logical pixels.
    pub width: f32,
}

/// Wrap text into line slices fitting within `max_width`.
pub fn wrap_text(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<LineLayout> {
    let mut lines = Vec::new();
    if text.is_empty() {
        lines.push(LineLayout {
            start: 0,
            end: 0,
            next_start: 0,
            width: 0.0,
        });
        return lines;
    }

    let mut para_start = 0;
    while para_start <= text.len() {
        let (para_str, has_newline, next_para_start) = match text[para_start..].find('\n') {
            Some(nl_pos) => {
                let abs_nl = para_start + nl_pos;
                (&text[para_start..abs_nl], true, abs_nl + 1)
            }
            None => (&text[para_start..], false, text.len() + 1),
        };

        if para_str.is_empty() {
            lines.push(LineLayout {
                start: para_start,
                end: para_start,
                next_start: if has_newline {
                    para_start + 1
                } else {
                    para_start
                },
                width: 0.0,
            });
        } else {
            let mut line_start = para_start;
            let para_end = para_start + para_str.len();

            while line_start < para_end {
                let remaining = &text[line_start..para_end];
                if max_width <= 0.0 || measure(remaining) <= max_width {
                    lines.push(LineLayout {
                        start: line_start,
                        end: para_end,
                        next_start: if has_newline { para_end + 1 } else { para_end },
                        width: measure(remaining),
                    });
                    break;
                }

                // Needs wrap. Find break point.
                let mut best_break = None;
                let mut current_offset = 0;
                for (char_idx, ch) in remaining.char_indices() {
                    let sub = &remaining[..char_idx + ch.len_utf8()];
                    let w = measure(sub);
                    if w > max_width {
                        break;
                    }
                    if ch.is_whitespace() {
                        best_break = Some(line_start + char_idx + ch.len_utf8());
                    }
                    current_offset = char_idx + ch.len_utf8();
                }

                let break_pos = match best_break {
                    Some(pos) if pos > line_start => pos,
                    _ => {
                        // Mid-word split: take as many chars as fit, at least one char
                        if current_offset > 0 {
                            line_start + current_offset
                        } else {
                            let first_char_len =
                                remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                            line_start + first_char_len
                        }
                    }
                };

                let line_text = &text[line_start..break_pos];
                lines.push(LineLayout {
                    start: line_start,
                    end: break_pos,
                    next_start: break_pos,
                    width: measure(line_text),
                });
                line_start = break_pos;
            }
        }

        if next_para_start > text.len() {
            break;
        }
        para_start = next_para_start;
    }

    if lines.is_empty() {
        lines.push(LineLayout {
            start: 0,
            end: 0,
            next_start: 0,
            width: 0.0,
        });
    }

    lines
}

/// Determine which line contains the cursor and the cursor's byte offset from that line's start.
pub fn cursor_line_and_col(lines: &[LineLayout], cursor: usize) -> (usize, usize) {
    if lines.is_empty() {
        return (0, 0);
    }
    for (i, line) in lines.iter().enumerate() {
        let is_last = i == lines.len() - 1;
        if is_last || cursor < line.next_start {
            let col = if cursor >= line.start {
                (cursor - line.start).min(line.end.saturating_sub(line.start))
            } else {
                0
            };
            return (i, col);
        }
    }
    let last_idx = lines.len() - 1;
    (
        last_idx,
        lines[last_idx].end.saturating_sub(lines[last_idx].start),
    )
}

fn pick_nearest_char_in_line(
    line: &LineLayout,
    text: &str,
    rel_x: f32,
    measure: impl Fn(&str) -> f32,
) -> usize {
    if line.start >= line.end || rel_x <= 0.0 {
        return line.start;
    }
    let line_str = &text[line.start..line.end];
    let total_w = measure(line_str);
    if rel_x >= total_w {
        return line.end;
    }

    let mut offsets = Vec::with_capacity(line_str.len() + 1);
    offsets.push((0, 0.0));
    for (idx, ch) in line_str.char_indices() {
        let next_idx = idx + ch.len_utf8();
        let adv = measure(&line_str[..next_idx]);
        offsets.push((next_idx, adv));
    }

    for window in offsets.windows(2) {
        let (idx_left, adv_left) = window[0];
        let (idx_right, adv_right) = window[1];
        if rel_x >= adv_left && rel_x <= adv_right {
            let mid = (adv_left + adv_right) * 0.5;
            let chosen = if rel_x < mid { idx_left } else { idx_right };
            return line.start + chosen;
        }
    }

    line.end
}

/// Reusable shared state for a text area, aliasing [`EditState`].
pub type TextAreaState = EditState;

/// Multi-line text edit control with cursor, soft-wrapping, and keyboard/mouse interaction.
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

impl TextArea {
    /// Create a new multi-line text area with initial text and bounding rectangle.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(EditState::new(text)),
            rect,
            padding: [6.0, 6.0],
            font_size: None,
            text_color: None,
            bg_color: Some([0.10, 0.13, 0.18, 0.85]),
            border_color: Some([0.15, 0.20, 0.28, 0.60]),
            border_thickness: 1.0,
            corner_radius: None,
            line_height: None,
            glow_style: None,
            placeholder: None,
        }
    }

    /// Attach an existing shared `TextAreaState` / `EditState`.
    pub fn with_state(mut self, state: Arc<TextAreaState>) -> Self {
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

    /// Set active focus state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.state.set_active(active);
    }

    /// Builder helper to set active focus state.
    pub fn with_active(self, is_active: bool) -> Self {
        self.state.set_active(is_active);
        self
    }

    /// Set inner padding `[horizontal, vertical]`.
    pub fn with_padding(mut self, padding: [f32; 2]) -> Self {
        self.padding = padding;
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

    /// Set line height multiplier (e.g. 1.3).
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Set placeholder text.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Available horizontal content width for line wrapping.
    #[inline]
    pub fn available_width(&self) -> f32 {
        (self.rect.w - 2.0 * self.padding[0]).max(0.0)
    }

    /// Calculate the character byte offset nearest to an absolute mouse position `[x, y]`.
    pub fn char_index_at_pos(&self, pos: [f32; 2]) -> usize {
        let text = self.text();
        if text.is_empty() {
            return 0;
        }
        let font_size = self.font_size.unwrap_or(14.0);
        let char_w = font_size * 0.55;
        let measure = |s: &str| s.chars().count() as f32 * char_w;

        let max_w = self.available_width();
        let lines = wrap_text(&text, max_w, measure);

        let line_step = font_size * self.line_height.unwrap_or(1.3);
        let pen_y = self.rect.y + self.padding[1];

        let line_idx = if pos[1] <= pen_y {
            0
        } else {
            let idx = ((pos[1] - pen_y) / line_step).floor() as usize;
            idx.min(lines.len() - 1)
        };

        let target_line = &lines[line_idx];
        let pen_x = self.rect.x + self.padding[0];
        let rel_x = pos[0] - pen_x;

        pick_nearest_char_in_line(target_line, &text, rel_x, measure)
    }

    /// Set the cursor to the character boundary nearest to the given position `[x, y]`.
    pub fn set_cursor_from_pos(&self, pos: [f32; 2]) {
        let idx = self.char_index_at_pos(pos);
        self.state.set_cursor(idx);
    }

    /// Obtain a borrowed handle to inspect the text area value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

impl DrawControl for TextArea {
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

        let max_w = self.available_width();
        let line_step = font_size * self.line_height.unwrap_or(1.3);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pen_origin_x = self.rect.x + self.padding[0];
        let pen_origin_y = self.rect.y + self.padding[1];

        let is_empty = text.is_empty();
        let display_placeholder = is_empty && self.placeholder.is_some();

        let measure = |s: &str| ctx.text_advance(s, font_size);

        if display_placeholder {
            let placeholder = self.placeholder.as_ref().unwrap();
            let p_lines = wrap_text(placeholder, max_w, measure);
            let dim_color = ctx.theme.muted_text_color;
            for (i, line) in p_lines.iter().enumerate() {
                let line_y = pen_origin_y + i as f32 * line_step;
                let line_text = &placeholder[line.start..line.end];
                ctx.draw_text(
                    list,
                    line_text,
                    [pen_origin_x, line_y],
                    font_size,
                    dim_color,
                );
            }
        }

        let lines = wrap_text(&text, max_w, measure);
        let text_col = self.text_color.unwrap_or(ctx.theme.text_color);

        if !is_empty {
            for (i, line) in lines.iter().enumerate() {
                let line_y = pen_origin_y + i as f32 * line_step;
                let line_text = &text[line.start..line.end];
                ctx.draw_text(list, line_text, [pen_origin_x, line_y], font_size, text_col);
            }
        }

        if self.is_active() || ctx.state.contains(NodeState::FOCUSED) {
            let cursor_pos = self.cursor();
            let safe_cursor = clamp_to_char_boundary(&text, cursor_pos);
            let (cursor_line_idx, cursor_col) = cursor_line_and_col(&lines, safe_cursor);
            let line = &lines[cursor_line_idx];
            let cursor_advance =
                ctx.text_advance(&text[line.start..line.start + cursor_col], font_size);
            let cursor_x = pen_origin_x + cursor_advance;
            let cursor_y = pen_origin_y + cursor_line_idx as f32 * line_step;
            let cursor_w = 1.5;

            list.push(SdfInstance {
                kind: SdfKind::Box,
                position: [cursor_x, cursor_y],
                size: [cursor_w, line_box_h],
                color: ctx.theme.accent_color,
                anim: 0,
            });

            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.rect, &style, &ctx.theme);
        }
    }
}

impl HostedControl for TextArea {
    /// A drag on it is its own (cursor positioning), not a scroll.
    fn claims_drag(&self, _part: ControlPart) -> bool {
        true
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if self.rect.contains(pos) {
            Some((ControlPart::Body, self.rect.normalized_x(pos[0])))
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
                self.set_cursor_from_pos(pos);
                Some(ControlAction::Activated { part: hit_part })
            }
            MouseEvent::Drag {
                current_pos,
                button: MouseButton::Left,
                ..
            } => {
                self.set_cursor_from_pos(current_pos);
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
                if *ch == '\r' || *ch == '\n' {
                    self.state.insert_char('\n');
                    return Some(ControlAction::TextInput { ch: '\n' });
                } else if !ch.is_control() {
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
                Key::Up => {
                    let text = self.text();
                    let cursor = self.cursor();
                    let font_size = self.font_size.unwrap_or(14.0);
                    let char_w = font_size * 0.55;
                    let measure = |s: &str| s.chars().count() as f32 * char_w;
                    let max_w = self.available_width();
                    let lines = wrap_text(&text, max_w, measure);
                    let (line_idx, col) = cursor_line_and_col(&lines, cursor);
                    if line_idx == 0 {
                        self.state.move_cursor_home();
                    } else {
                        let current_line = &lines[line_idx];
                        let col_adv = measure(&text[current_line.start..current_line.start + col]);
                        let target_line = &lines[line_idx - 1];
                        let new_cursor =
                            pick_nearest_char_in_line(target_line, &text, col_adv, measure);
                        self.state.set_cursor(new_cursor);
                    }
                    return Some(ControlAction::StepLeft);
                }
                Key::Down => {
                    let text = self.text();
                    let cursor = self.cursor();
                    let font_size = self.font_size.unwrap_or(14.0);
                    let char_w = font_size * 0.55;
                    let measure = |s: &str| s.chars().count() as f32 * char_w;
                    let max_w = self.available_width();
                    let lines = wrap_text(&text, max_w, measure);
                    let (line_idx, col) = cursor_line_and_col(&lines, cursor);
                    if line_idx >= lines.len() - 1 {
                        self.state.move_cursor_end();
                    } else {
                        let current_line = &lines[line_idx];
                        let col_adv = measure(&text[current_line.start..current_line.start + col]);
                        let target_line = &lines[line_idx + 1];
                        let new_cursor =
                            pick_nearest_char_in_line(target_line, &text, col_adv, measure);
                        self.state.set_cursor(new_cursor);
                    }
                    return Some(ControlAction::StepRight);
                }
                Key::Home => {
                    let text = self.text();
                    let cursor = self.cursor();
                    let font_size = self.font_size.unwrap_or(14.0);
                    let char_w = font_size * 0.55;
                    let measure = |s: &str| s.chars().count() as f32 * char_w;
                    let max_w = self.available_width();
                    let lines = wrap_text(&text, max_w, measure);
                    let (line_idx, _) = cursor_line_and_col(&lines, cursor);
                    self.state.set_cursor(lines[line_idx].start);
                    return Some(ControlAction::StepLeft);
                }
                Key::End => {
                    let text = self.text();
                    let cursor = self.cursor();
                    let font_size = self.font_size.unwrap_or(14.0);
                    let char_w = font_size * 0.55;
                    let measure = |s: &str| s.chars().count() as f32 * char_w;
                    let max_w = self.available_width();
                    let lines = wrap_text(&text, max_w, measure);
                    let (line_idx, _) = cursor_line_and_col(&lines, cursor);
                    self.state.set_cursor(lines[line_idx].end);
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
                    self.state.insert_char('\n');
                    return Some(ControlAction::TextInput { ch: '\n' });
                }
                Key::Space => {
                    self.state.insert_char(' ');
                    return Some(ControlAction::TextInput { ch: ' ' });
                }
                Key::Char(ch) => {
                    if *ch == '\r' || *ch == '\n' {
                        self.state.insert_char('\n');
                        return Some(ControlAction::TextInput { ch: '\n' });
                    } else if !ch.is_control() {
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

impl ReadValue<String> for TextArea {
    fn read_value(&self) -> String {
        self.text()
    }
}
