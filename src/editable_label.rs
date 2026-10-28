//! A label edited in place: text with a pencil that turns it into a
//! one-line field with a check and an optional X
//! (docs/design/20261028_05_EDITABLE_LABEL_PLAN.md).

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO};

use crate::control::HostedControl;
use crate::edit::{Edit, EditState};
use crate::event::{
    ControlAction, ControlPart, EventContext, FocusEvent, Key, KeyboardEvent, MouseButton,
    MouseEvent,
};
use crate::flags::{NodeFlags, NodeState};
use crate::handle::{ControlHandle, ReadValue};
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::theme::ControlTheme;
use crate::traits::{DrawContext, DrawControl};

/// What happened to an editable label, queued by its state and drained
/// with [`EditableLabelState::take_events`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditableLabelEvent {
    /// Editing began; the draft holds the text.
    Started,
    /// The draft was kept; it is now the text.
    Confirmed(String),
    /// The draft was thrown away; the text is unchanged.
    Cancelled,
    /// The X emptied the draft; still editing.
    Cleared,
}

/// The shared state of an [`EditableLabel`]: the committed text, the draft
/// while editing, and the event queue.
#[derive(Debug)]
pub struct EditableLabelState {
    text: Mutex<String>,
    draft: Arc<EditState>,
    editing: AtomicBool,
    events: Mutex<Vec<EditableLabelEvent>>,
}

impl EditableLabelState {
    /// A state showing `text`, not editing.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Mutex::new(text.into()),
            draft: Arc::new(EditState::new("")),
            editing: AtomicBool::new(false),
            events: Mutex::new(Vec::new()),
        }
    }

    /// The committed text.
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Replace the committed text. Queues nothing and leaves a draft alone.
    pub fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
    }

    /// Whether the label is being edited.
    pub fn is_editing(&self) -> bool {
        self.editing.load(Ordering::Relaxed)
    }

    /// The draft while editing.
    pub fn draft(&self) -> Option<String> {
        self.is_editing().then(|| self.draft.text())
    }

    /// The field's `EditState` (the draft, its cursor).
    pub fn draft_state(&self) -> &Arc<EditState> {
        &self.draft
    }

    /// Replace the draft, cursor at its end (a soft keyboard's whole-text
    /// update). Ignored unless editing.
    pub fn set_draft(&self, text: impl Into<String>) {
        if self.is_editing() {
            self.draft.set_text(text);
            self.draft.move_cursor_end();
        }
    }

    /// Start editing: the draft is the text, cursor at the end. Queues
    /// `Started`; `false` if already editing.
    pub fn begin_edit(&self) -> bool {
        if self.is_editing() {
            return false;
        }
        self.draft.set_text(self.text());
        self.draft.move_cursor_end();
        self.draft.set_active(true);
        self.editing.store(true, Ordering::Relaxed);
        self.push(EditableLabelEvent::Started);
        true
    }

    /// Keep the draft as the text and stop editing. Queues
    /// `Confirmed(text)`; `None` if not editing.
    pub fn confirm(&self) -> Option<String> {
        if !self.is_editing() {
            return None;
        }
        let value = self.draft.text();
        *self.text.lock().unwrap() = value.clone();
        self.stop();
        self.push(EditableLabelEvent::Confirmed(value.clone()));
        Some(value)
    }

    /// Throw the draft away and stop editing. Queues `Cancelled`; `false`
    /// if not editing.
    pub fn cancel(&self) -> bool {
        if !self.is_editing() {
            return false;
        }
        self.stop();
        self.push(EditableLabelEvent::Cancelled);
        true
    }

    /// Empty the draft and keep editing. Queues `Cleared`; `false` if not
    /// editing or the draft is already empty.
    pub fn clear(&self) -> bool {
        if !self.is_editing() || self.draft.text().is_empty() {
            return false;
        }
        self.draft.set_text("");
        self.draft.set_cursor(0);
        self.push(EditableLabelEvent::Cleared);
        true
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&self) -> Vec<EditableLabelEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }

    fn stop(&self) {
        self.editing.store(false, Ordering::Relaxed);
        self.draft.set_active(false);
    }

    fn push(&self, event: EditableLabelEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// The defaults every editable label starts from, in
/// `ControlTheme::editable_label` (the theming standard, spec section 8.5).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelTheme {
    /// Each icon button's width; its height is the control's.
    pub button_width: f32,
    /// The side of the square an icon is drawn in.
    pub icon_size: f32,
    /// The icons' stroke.
    pub icon_thickness: f32,
    /// Space between the text or field and the buttons.
    pub gap: f32,
    /// `None`: the theme's `muted_text_color`.
    pub pencil_color: Option<[f32; 4]>,
    /// `None`: the theme's `accent_color`.
    pub confirm_color: Option<[f32; 4]>,
    /// `None`: the theme's `muted_text_color`.
    pub clear_color: Option<[f32; 4]>,
    /// `None`: the theme's `muted_text_color`.
    pub placeholder_color: Option<[f32; 4]>,
    /// The field's fill (`Edit`'s).
    pub field_bg: [f32; 4],
    /// The field's outline (`Edit`'s).
    pub field_border: [f32; 4],
    pub field_border_thickness: f32,
}

impl Default for EditableLabelTheme {
    fn default() -> Self {
        Self {
            button_width: 32.0,
            icon_size: 14.0,
            icon_thickness: 1.6,
            gap: 4.0,
            pencil_color: None,
            confirm_color: None,
            clear_color: None,
            placeholder_color: None,
            field_bg: [0.10, 0.13, 0.18, 0.85],
            field_border: [0.15, 0.20, 0.28, 0.60],
            field_border_thickness: 1.0,
        }
    }
}

/// One editable label's overrides of its theme (`None` takes the theme's
/// value) and its own settings.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelOptions {
    pub button_width: Option<f32>,
    pub icon_size: Option<f32>,
    pub icon_thickness: Option<f32>,
    pub gap: Option<f32>,
    pub pencil_color: Option<[f32; 4]>,
    pub confirm_color: Option<[f32; 4]>,
    pub clear_color: Option<[f32; 4]>,
    pub placeholder_color: Option<[f32; 4]>,
    pub field_bg: Option<[f32; 4]>,
    pub field_border: Option<[f32; 4]>,
    pub field_border_thickness: Option<f32>,
    /// `None`: the theme's `default_font_size`.
    pub font_size: Option<f32>,
    /// `None`: the theme's `text_color`.
    pub text_color: Option<[f32; 4]>,
    /// Drawn, in the placeholder colour, when the text or draft is empty.
    pub placeholder: Option<String>,
    /// Show the X while editing a non-empty draft.
    pub clear: bool,
    /// Text only: no pencil, never edits, not a Tab stop.
    pub read_only: bool,
    /// A tap on the text starts editing too.
    pub label_edits: bool,
}

macro_rules! setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl EditableLabelOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    setter!(button_width: f32);
    setter!(icon_size: f32);
    setter!(icon_thickness: f32);
    setter!(gap: f32);
    setter!(pencil_color: [f32; 4]);
    setter!(confirm_color: [f32; 4]);
    setter!(clear_color: [f32; 4]);
    setter!(placeholder_color: [f32; 4]);
    setter!(field_bg: [f32; 4]);
    setter!(field_border: [f32; 4]);
    setter!(field_border_thickness: f32);
    setter!(font_size: f32);
    setter!(text_color: [f32; 4]);

    /// The text drawn when the text or draft is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Show the X while editing a non-empty draft.
    pub fn clear(mut self, on: bool) -> Self {
        self.clear = on;
        self
    }

    /// Text only, never edited.
    pub fn read_only(mut self, on: bool) -> Self {
        self.read_only = on;
        self
    }

    /// A tap on the text starts editing too.
    pub fn label_edits(mut self, on: bool) -> Self {
        self.label_edits = on;
        self
    }

    /// Each `Some` wins; each `None` takes the theme's value.
    pub fn with_theme(&self, theme: &EditableLabelTheme) -> EditableLabelResolved {
        EditableLabelResolved {
            button_width: self.button_width.unwrap_or(theme.button_width),
            icon_size: self.icon_size.unwrap_or(theme.icon_size),
            icon_thickness: self.icon_thickness.unwrap_or(theme.icon_thickness),
            gap: self.gap.unwrap_or(theme.gap),
            pencil_color: self.pencil_color.or(theme.pencil_color),
            confirm_color: self.confirm_color.or(theme.confirm_color),
            clear_color: self.clear_color.or(theme.clear_color),
            placeholder_color: self.placeholder_color.or(theme.placeholder_color),
            field_bg: self.field_bg.unwrap_or(theme.field_bg),
            field_border: self.field_border.unwrap_or(theme.field_border),
            field_border_thickness: self
                .field_border_thickness
                .unwrap_or(theme.field_border_thickness),
        }
    }
}

/// An editable label's options resolved against its theme: what it reads.
/// Colours left `None` are the `ControlTheme`'s flat colours, read at draw.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelResolved {
    pub button_width: f32,
    pub icon_size: f32,
    pub icon_thickness: f32,
    pub gap: f32,
    pub pencil_color: Option<[f32; 4]>,
    pub confirm_color: Option<[f32; 4]>,
    pub clear_color: Option<[f32; 4]>,
    pub placeholder_color: Option<[f32; 4]>,
    pub field_bg: [f32; 4],
    pub field_border: [f32; 4],
    pub field_border_thickness: f32,
}

/// Where an editable label's parts are, for its current mode.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelParts {
    /// The text (display) or the field (editing).
    pub text: Rect,
    /// The pencil (display) or the check (editing); `None` read-only.
    pub button: Option<Rect>,
    /// The X, while it is shown.
    pub clear: Option<Rect>,
}

/// A label edited in place.
#[derive(Clone, Debug)]
pub struct EditableLabel {
    /// Shared state: text, draft, events.
    pub state: Arc<EditableLabelState>,
    /// Bounding rectangle.
    pub rect: Rect,
    /// Theme overrides and settings.
    pub options: EditableLabelOptions,
    /// The resolved values of the last draw, for hit testing.
    last: Cell<EditableLabelResolved>,
}

/// Horizontal text padding, as `Label` and `Edit` use.
const PAD_H: f32 = 4.0;

impl EditableLabel {
    /// The pencil, in display mode.
    pub const PENCIL: ControlPart = ControlPart::Custom(0x454C_0001);
    /// The check, while editing.
    pub const CONFIRM: ControlPart = ControlPart::Custom(0x454C_0002);
    /// The X, while it is shown.
    pub const CLEAR: ControlPart = ControlPart::Custom(0x454C_0003);
    /// The text in display mode.
    pub const TEXT: ControlPart = ControlPart::Label;
    /// The field while editing (`Edit`'s part).
    pub const FIELD: ControlPart = ControlPart::NumericBox;

    /// A label showing `text` in `rect`.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        let options = EditableLabelOptions::default();
        let last = Cell::new(options.with_theme(&EditableLabelTheme::default()));
        Self {
            state: Arc::new(EditableLabelState::new(text)),
            rect,
            options,
            last,
        }
    }

    /// Attach an existing shared state.
    pub fn with_state(mut self, state: Arc<EditableLabelState>) -> Self {
        self.state = state;
        self
    }

    /// Replace the options.
    pub fn with_options(mut self, options: EditableLabelOptions) -> Self {
        self.last
            .set(options.with_theme(&EditableLabelTheme::default()));
        self.options = options;
        self
    }

    /// Show the X while editing a non-empty draft.
    pub fn with_clear(mut self, on: bool) -> Self {
        self.options.clear = on;
        self
    }

    /// The text drawn when the text or draft is empty.
    pub fn with_placeholder(mut self, text: impl Into<String>) -> Self {
        self.options.placeholder = Some(text.into());
        self
    }

    /// Text only, never edited, not a Tab stop. Set before adding it to a
    /// router: the router reads the flags once.
    pub fn with_read_only(mut self, on: bool) -> Self {
        self.options.read_only = on;
        self
    }

    /// A tap on the text starts editing too.
    pub fn with_label_edits(mut self, on: bool) -> Self {
        self.options.label_edits = on;
        self
    }

    /// Override the theme's font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.options.font_size = Some(size);
        self
    }

    /// Override the theme's text colour.
    pub fn with_text_color(mut self, color: [f32; 4]) -> Self {
        self.options.text_color = Some(color);
        self
    }

    /// The committed text.
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// Whether it is being edited.
    pub fn is_editing(&self) -> bool {
        self.state.is_editing()
    }

    /// What the text line shows now: the draft or text, or the placeholder
    /// when that is empty.
    pub fn shown_text(&self) -> String {
        let text = self.state.draft().unwrap_or_else(|| self.state.text());
        if text.is_empty() {
            self.options.placeholder.clone().unwrap_or_default()
        } else {
            text
        }
    }

    /// Whether the placeholder is what is shown.
    pub fn shows_placeholder(&self) -> bool {
        let text = self.state.draft().unwrap_or_else(|| self.state.text());
        text.is_empty() && self.options.placeholder.is_some()
    }

    /// The options resolved against `theme.editable_label`.
    pub fn resolved(&self, theme: &ControlTheme) -> EditableLabelResolved {
        self.options.with_theme(&theme.editable_label)
    }

    /// Whether the X is shown: enabled, editing and the draft not empty.
    pub fn clear_shown(&self) -> bool {
        self.options.clear
            && !self.options.read_only
            && self.state.draft().is_some_and(|d| !d.is_empty())
    }

    /// The parts' rects for the current mode.
    pub fn parts(&self, resolved: &EditableLabelResolved) -> EditableLabelParts {
        let r = self.rect;
        if self.options.read_only {
            return EditableLabelParts {
                text: r,
                button: None,
                clear: None,
            };
        }
        let bw = resolved.button_width.clamp(0.0, r.w.max(0.0));
        let mut right = r.right() - bw;
        let button = Rect::new(right, r.y, bw, r.h);
        let clear = if self.is_editing() && self.clear_shown() {
            let start = (right - bw).max(r.x);
            let c = Rect::new(start, r.y, right - start, r.h);
            right = start;
            Some(c)
        } else {
            None
        };
        let text_w = (right - resolved.gap - r.x).max(0.0);
        EditableLabelParts {
            text: Rect::new(r.x, r.y, text_w, r.h),
            button: Some(button),
            clear,
        }
    }

    /// The field over the draft state, in `rect`.
    fn field(&self, resolved: &EditableLabelResolved, rect: Rect) -> Edit {
        Edit {
            state: self.state.draft_state().clone(),
            rect,
            align: TextAlign::Left,
            font_size: self.options.font_size,
            text_color: self.options.text_color,
            bg_color: Some(resolved.field_bg),
            border_color: Some(resolved.field_border),
            border_thickness: resolved.field_border_thickness,
            corner_radius: None,
            glow_style: None,
        }
    }

    /// The field at its place for the last drawn values.
    fn last_field(&self) -> Edit {
        let resolved = self.last.get();
        self.field(&resolved, self.parts(&resolved).text)
    }

    /// Obtain a borrowed handle to read the committed text.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

fn at(c: [f32; 2], s: f32, dx: f32, dy: f32) -> [f32; 2] {
    [c[0] + dx * s, c[1] + dy * s]
}

/// A pencil pointing down-left: six segments.
fn draw_pencil(list: &mut DrawList, c: [f32; 2], s: f32, t: f32, col: [f32; 4]) {
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let w = 0.16;
    // n: across the body (down-right); the body runs from top-right down-left.
    let side = |p: [f32; 2], k: f32| [p[0] + h * w * s * k, p[1] + h * w * s * k];
    let top = at(c, s, 0.36, -0.36);
    let ferrule = at(c, s, 0.22, -0.22);
    let base = at(c, s, -0.16, 0.16);
    let tip = at(c, s, -0.38, 0.38);
    list.push_line(side(top, 1.0), side(base, 1.0), t, col);
    list.push_line(side(top, -1.0), side(base, -1.0), t, col);
    list.push_line(side(top, 1.0), side(top, -1.0), t, col);
    list.push_line(side(ferrule, 1.0), side(ferrule, -1.0), t, col);
    list.push_line(side(base, 1.0), tip, t, col);
    list.push_line(side(base, -1.0), tip, t, col);
}

/// A check mark: two segments.
fn draw_check(list: &mut DrawList, c: [f32; 2], s: f32, t: f32, col: [f32; 4]) {
    let knee = at(c, s, -0.10, 0.30);
    list.push_line(at(c, s, -0.38, 0.02), knee, t, col);
    list.push_line(knee, at(c, s, 0.38, -0.30), t, col);
}

/// An X: two segments.
fn draw_x(list: &mut DrawList, c: [f32; 2], s: f32, t: f32, col: [f32; 4]) {
    list.push_line(at(c, s, -0.30, -0.30), at(c, s, 0.30, 0.30), t, col);
    list.push_line(at(c, s, -0.30, 0.30), at(c, s, 0.30, -0.30), t, col);
}

impl DrawControl for EditableLabel {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let resolved = self.resolved(&ctx.theme);
        self.last.set(resolved);
        let parts = self.parts(&resolved);
        let font_size = ctx.font_size(self.options.font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let muted = ctx.theme.muted_text_color;
        let placeholder_col = resolved.placeholder_color.unwrap_or(muted);
        let s = resolved.icon_size;
        let t = resolved.icon_thickness;

        if self.is_editing() {
            let field = self.field(&resolved, parts.text);
            field.draw(list, ctx);
            if self.shows_placeholder() {
                let pen_y = parts.text.y + (parts.text.h - line_box_h) * 0.5;
                ctx.draw_text(
                    list,
                    &self.shown_text(),
                    [parts.text.x + PAD_H, pen_y],
                    font_size,
                    placeholder_col,
                );
            }
            if let Some(clear) = parts.clear {
                let col = resolved.clear_color.unwrap_or(muted);
                draw_x(list, clear.center(), s, t, col);
            }
            if let Some(button) = parts.button {
                let col = resolved.confirm_color.unwrap_or(ctx.theme.accent_color);
                draw_check(list, button.center(), s, t, col);
            }
            return;
        }

        let col = if self.shows_placeholder() {
            placeholder_col
        } else {
            ctx.text_color(self.options.text_color)
        };
        let pen_y = parts.text.y + (parts.text.h - line_box_h) * 0.5;
        ctx.draw_text(
            list,
            &self.shown_text(),
            [parts.text.x + PAD_H, pen_y],
            font_size,
            col,
        );
        if let Some(button) = parts.button {
            let col = resolved.pencil_color.unwrap_or(muted);
            draw_pencil(list, button.center(), s, t, col);
        }
        if ctx.state.contains(NodeState::FOCUSED) {
            draw_glowing_ring(list, self.rect, &GlowRingStyle::default(), &ctx.theme);
        }
    }
}

impl HostedControl for EditableLabel {
    fn default_flags(&self) -> NodeFlags {
        if self.options.read_only {
            NodeFlags::NONE
        } else {
            NodeFlags::FOCUSABLE
        }
    }

    /// A drag in the field moves its cursor, not a scroll pane.
    fn claims_drag(&self, part: ControlPart) -> bool {
        part == Self::FIELD && self.is_editing()
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let parts = self.parts(&self.last.get());
        let editing = self.is_editing();
        let part = if parts.button.is_some_and(|b| b.contains(pos)) {
            if editing { Self::CONFIRM } else { Self::PENCIL }
        } else if parts.clear.is_some_and(|c| c.contains(pos)) {
            Self::CLEAR
        } else if parts.text.contains(pos) {
            if editing { Self::FIELD } else { Self::TEXT }
        } else {
            ControlPart::Body
        };
        Some((part, self.rect.normalized_x(pos[0])))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        if self.options.read_only {
            return None;
        }
        let part = ctx.part;
        let editing = self.is_editing();
        let activated = Some(ControlAction::Activated { part });
        match *event {
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            } if !editing => {
                let edits =
                    part == Self::PENCIL || (part == Self::TEXT && self.options.label_edits);
                if edits && self.state.begin_edit() {
                    activated
                } else {
                    None
                }
            }
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            } if part == Self::CONFIRM => {
                self.state.confirm();
                activated
            }
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            } if part == Self::CLEAR => {
                if self.clear_shown() && self.state.clear() {
                    activated
                } else {
                    None
                }
            }
            MouseEvent::Down { .. } | MouseEvent::Click { .. } | MouseEvent::Drag { .. }
                if editing && part == Self::FIELD =>
            {
                self.last_field().handle_mouse(event, ctx)
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        if self.options.read_only || !event.is_down() {
            return None;
        }
        let key = event.key();
        if !self.is_editing() {
            if let Some(Key::Enter | Key::Space) = key
                && self.state.begin_edit()
            {
                return Some(ControlAction::Activated { part: Self::PENCIL });
            }
            return None;
        }
        match key {
            Some(Key::Enter) => {
                self.state.confirm();
                Some(ControlAction::Activated {
                    part: Self::CONFIRM,
                })
            }
            Some(Key::Escape | Key::Back) => {
                self.state.cancel();
                Some(ControlAction::Activated {
                    part: ControlPart::Body,
                })
            }
            _ => self.last_field().handle_keyboard(event, ctx),
        }
    }

    /// Losing the focus while editing (the router's Escape, a press
    /// elsewhere, Tab) cancels.
    fn handle_focus(&self, event: FocusEvent) {
        if event == FocusEvent::Lost {
            self.state.cancel();
        }
    }
}

impl ReadValue<String> for EditableLabel {
    fn read_value(&self) -> String {
        self.text()
    }
}
