//! A popup menu of actions, opened and closed by calls on its shared state
//! and shown as a layer of the router's modal stack
//! (docs/design/20261028_06_POPUP_MENU_PLAN.md), with toggle items and rows
//! of cells in shared columns
//! (docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md).

use std::sync::{Arc, Mutex, MutexGuard};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseEvent};
use crate::flags::NodeFlags;
use crate::modal::{
    DismissOn, LayerContext, LayerFilter, LayerShape, Modal, ModalError, ModalLayerFilter,
    ModalOptions, ModalState, ModalTransition,
};
use crate::rect::{Rect, TextAlign};
use crate::router::Router;
use crate::scrollbar::cut_to_fit;
use crate::symbol::Symbol;
use crate::theme::ControlTheme;
use crate::traits::{DrawContext, DrawControl};

/// What a menu item is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopupItemKind {
    /// Chosen by a tap or Enter while enabled.
    Action,
    /// A small title over the items below it; takes no input.
    Heading,
    /// A line between groups; takes no input.
    Separator,
    /// Chosen like an action, but flips `checked` and keeps the menu open.
    Toggle,
}

/// One row of a popup menu.
#[derive(Clone, Debug, PartialEq)]
pub struct PopupItem {
    /// What `PopupMenuEvent::Chosen` (or `Toggled`) reports.
    pub id: Symbol,
    pub label: String,
    /// A disabled action or toggle is drawn dim and takes no input.
    pub enabled: bool,
    /// `Some(true)` draws a check mark; `Some(_)` on any item reserves the
    /// check column for every item.
    pub checked: Option<bool>,
    pub kind: PopupItemKind,
}

impl PopupItem {
    /// An enabled action.
    pub fn action(id: impl Into<Symbol>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            checked: None,
            kind: PopupItemKind::Action,
        }
    }

    /// An enabled toggle, on or off.
    pub fn toggle(id: impl Into<Symbol>, label: impl Into<String>, checked: bool) -> Self {
        Self {
            checked: Some(checked),
            kind: PopupItemKind::Toggle,
            ..Self::action(id, label)
        }
    }

    /// A heading (id `""`).
    pub fn heading(label: impl Into<String>) -> Self {
        Self {
            kind: PopupItemKind::Heading,
            ..Self::action("", label)
        }
    }

    /// A separator (id and label `""`).
    pub fn separator() -> Self {
        Self {
            kind: PopupItemKind::Separator,
            ..Self::action("", "")
        }
    }

    /// The same item, enabled or not.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The same item with this check state.
    #[must_use]
    pub fn with_checked(mut self, checked: Option<bool>) -> Self {
        self.checked = checked;
        self
    }

    /// This item with cells, as a row.
    #[must_use]
    pub fn with_cells(self, cells: Vec<PopupCell>) -> PopupRow {
        PopupRow::new(self).with_cells(cells)
    }

    /// Whether it can be chosen: an enabled action or toggle.
    pub fn is_choosable(&self) -> bool {
        self.enabled && matches!(self.kind, PopupItemKind::Action | PopupItemKind::Toggle)
    }

    /// Whether it is drawn at `disabled_alpha`: a disabled action or toggle.
    fn is_dimmed(&self) -> bool {
        !self.enabled && matches!(self.kind, PopupItemKind::Action | PopupItemKind::Toggle)
    }
}

/// How wide a cell's column is.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum PopupCellWidth {
    /// As wide as the column's widest cell.
    #[default]
    Auto,
    /// This many px.
    Fixed(f32),
    /// The room left over; shrinks first.
    Flex,
}

/// One cell of a row, drawn in its column.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupCell {
    /// One line; anything from the first `\n` on is cut with "…".
    pub text: String,
    /// Where the text sits in its column (default `Left`).
    pub align: TextAlign,
    /// How the column is sized (default `Auto`).
    pub width: PopupCellWidth,
    /// Drawn in `dim_text_color`.
    pub dim: bool,
    /// Times the menu's font size (default 1.0), clamped to 0.25..=4.0.
    pub scale: f32,
}

impl PopupCell {
    /// Left-aligned, `Auto`, not dim, scale 1.0.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            align: TextAlign::Left,
            width: PopupCellWidth::Auto,
            dim: false,
            scale: 1.0,
        }
    }

    /// The same cell with this alignment.
    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// The same cell with this width.
    #[must_use]
    pub fn width(mut self, width: PopupCellWidth) -> Self {
        self.width = width;
        self
    }

    /// `width(PopupCellWidth::Fixed(w))`.
    #[must_use]
    pub fn fixed(self, w: f32) -> Self {
        self.width(PopupCellWidth::Fixed(w))
    }

    /// `width(PopupCellWidth::Flex)`.
    #[must_use]
    pub fn flex(self) -> Self {
        self.width(PopupCellWidth::Flex)
    }

    /// The same cell, dim or not.
    #[must_use]
    pub fn dim(mut self, dim: bool) -> Self {
        self.dim = dim;
        self
    }

    /// The same cell at this multiple of the menu's font size.
    #[must_use]
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// The scale as used: clamped to 0.25..=4.0, 1.0 when not finite.
    fn used_scale(&self) -> f32 {
        if self.scale.is_finite() {
            self.scale.clamp(0.25, 4.0)
        } else {
            1.0
        }
    }
}

/// A menu row: an item and, optionally, cells drawn instead of its label.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupRow {
    pub item: PopupItem,
    /// Empty: the label is drawn, as for a plain item.
    pub cells: Vec<PopupCell>,
}

impl PopupRow {
    /// A row of `item` with no cells.
    pub fn new(item: PopupItem) -> Self {
        Self {
            item,
            cells: Vec::new(),
        }
    }

    /// The same row with these cells.
    #[must_use]
    pub fn with_cells(mut self, cells: Vec<PopupCell>) -> Self {
        self.cells = cells;
        self
    }

    /// The cells it draws: none for a separator.
    fn drawn_cells(&self) -> &[PopupCell] {
        if self.item.kind == PopupItemKind::Separator {
            &[]
        } else {
            &self.cells
        }
    }
}

impl From<PopupItem> for PopupRow {
    fn from(item: PopupItem) -> Self {
        Self::new(item)
    }
}

/// What happened in a menu, queued by its state and drained with
/// [`PopupMenuState::take_events`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopupMenuEvent {
    /// This action was chosen; the menu closed.
    Chosen(Symbol),
    /// The menu closed without a choice: an outside tap, Escape, Back, or
    /// its modal entry closed by anything but [`PopupMenuState::close`].
    Dismissed,
    /// A toggle was chosen; the menu stays open. `checked` is its new state.
    Toggled { id: Symbol, checked: bool },
}

/// Where an open menu and its items are.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuLayout {
    /// The menu's frame.
    pub frame: Rect,
    /// One rect per item, in order.
    pub items: Vec<Rect>,
    /// Whether it opened above its anchor.
    pub above: bool,
    /// Each column's screen x and width; empty when no row has cells.
    pub columns: Vec<[f32; 2]>,
}

#[derive(Debug, Default)]
struct Inner {
    open: bool,
    anchor: Rect,
    rows: Vec<PopupRow>,
    highlight: Option<usize>,
    pressed: Option<usize>,
    events: Vec<PopupMenuEvent>,
    layout: Option<PopupMenuLayout>,
    binding: Option<(Arc<ModalState>, Symbol)>,
}

impl Inner {
    fn choosable(&self, index: usize) -> bool {
        self.rows.get(index).is_some_and(|r| r.item.is_choosable())
    }

    /// The next choosable item after `from` going `forward`, wrapping.
    fn step(&self, forward: bool) -> Option<usize> {
        let n = self.rows.len();
        if n == 0 {
            return None;
        }
        let start = match (self.highlight, forward) {
            (Some(h), true) => h + 1,
            (Some(h), false) => h + n - 1,
            (None, true) => 0,
            (None, false) => n - 1,
        };
        (0..n)
            .map(|k| {
                if forward {
                    (start + k) % n
                } else {
                    (start + n - k) % n
                }
            })
            .find(|&i| self.choosable(i))
    }
}

/// The shared state of a [`PopupMenu`]: whether it is open, its anchor and
/// items, the highlight and the event queue.
#[derive(Debug, Default)]
pub struct PopupMenuState {
    inner: Mutex<Inner>,
}

impl PopupMenuState {
    /// A closed menu with no items.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// Bind to a modal stack's state under the entry `id`
    /// (`Router::attach_popup_menu`).
    fn bind(&self, modal: Arc<ModalState>, id: Symbol) {
        self.lock().binding = Some((modal, id));
    }

    /// Open the menu at `anchor` with `items`, or, while open, replace them.
    /// Shown once the menu is attached to a router's modal stack.
    pub fn open(&self, anchor: Rect, items: Vec<PopupItem>) {
        self.open_rows(anchor, items.into_iter().map(PopupRow::from).collect());
    }

    /// As [`open`](Self::open), with rows.
    pub fn open_rows(&self, anchor: Rect, rows: Vec<PopupRow>) {
        let binding = {
            let mut inner = self.lock();
            inner.open = true;
            inner.anchor = anchor;
            inner.rows = rows;
            inner.highlight = None;
            inner.pressed = None;
            inner.layout = None;
            inner.binding.clone()
        };
        if let Some((modal, id)) = binding
            && !modal.contains(id)
        {
            modal.push_with(id, ModalTransition::Instant);
        }
    }

    /// Close the menu. Queues nothing: the app knows.
    pub fn close(&self) {
        let binding = {
            let mut inner = self.lock();
            inner.open = false;
            inner.highlight = None;
            inner.pressed = None;
            inner.binding.clone()
        };
        if let Some((modal, id)) = binding {
            modal.close(id);
        }
    }

    /// A menu that is open but whose entry is gone was dismissed by the
    /// stack (or the app's stack calls): close it and queue `Dismissed`.
    fn reconcile(&self) {
        let mut inner = self.lock();
        let gone = inner.open
            && inner
                .binding
                .as_ref()
                .is_some_and(|(modal, id)| !modal.contains(*id));
        if gone {
            inner.open = false;
            inner.highlight = None;
            inner.pressed = None;
            inner.events.push(PopupMenuEvent::Dismissed);
        }
    }

    /// Whether the menu is open.
    pub fn is_open(&self) -> bool {
        self.reconcile();
        self.lock().open
    }

    /// Every event since the last call, in order.
    pub fn take_events(&self) -> Vec<PopupMenuEvent> {
        self.reconcile();
        std::mem::take(&mut self.lock().events)
    }

    /// The rect it was last opened at.
    pub fn anchor(&self) -> Rect {
        self.lock().anchor
    }

    /// The items it was last opened with, toggles as flipped since.
    pub fn items(&self) -> Vec<PopupItem> {
        self.lock().rows.iter().map(|r| r.item.clone()).collect()
    }

    /// The rows it was last opened with, toggles as flipped since.
    pub fn rows(&self) -> Vec<PopupRow> {
        self.lock().rows.clone()
    }

    /// The highlighted item's index.
    pub fn highlighted(&self) -> Option<usize> {
        self.lock().highlight
    }

    /// The layout of the last draw since it was opened.
    pub fn layout(&self) -> Option<PopupMenuLayout> {
        self.lock().layout.clone()
    }

    fn set_layout(&self, layout: PopupMenuLayout) {
        self.lock().layout = Some(layout);
    }

    fn set_highlight(&self, index: Option<usize>) {
        let mut inner = self.lock();
        inner.highlight = index.filter(|&i| inner.choosable(i));
    }

    fn move_highlight(&self, forward: bool) {
        let mut inner = self.lock();
        inner.highlight = inner.step(forward);
    }

    fn highlight_end(&self, last: bool) {
        let mut inner = self.lock();
        inner.highlight = None;
        inner.highlight = inner.step(!last);
    }

    /// Choose item `index`: an action queues `Chosen` and closes; a toggle
    /// flips its `checked`, queues `Toggled` and stays open with its
    /// highlight. False if it is not an enabled action or toggle or the menu
    /// is closed.
    fn activate(&self, index: usize) -> bool {
        let binding = {
            let mut inner = self.lock();
            if !inner.open || !inner.choosable(index) {
                return false;
            }
            let item = &mut inner.rows[index].item;
            let id = item.id;
            if item.kind == PopupItemKind::Toggle {
                let checked = !item.checked.unwrap_or(false);
                item.checked = Some(checked);
                inner.pressed = None;
                inner.events.push(PopupMenuEvent::Toggled { id, checked });
                return true;
            }
            inner.open = false;
            inner.highlight = None;
            inner.pressed = None;
            inner.events.push(PopupMenuEvent::Chosen(id));
            inner.binding.clone()
        };
        if let Some((modal, id)) = binding {
            modal.close(id);
        }
        true
    }
}

/// `ControlTheme::popup_menu` (the theming standard, spec section 8.5).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuTheme {
    /// The menu's fill.
    pub bg: [f32; 4],
    /// The menu's outline.
    pub border: [f32; 4],
    /// 0: no outline.
    pub border_thickness: f32,
    pub corner_radius: f32,
    /// Each action's and heading's height.
    pub item_height: f32,
    /// From the frame to the items, on every side.
    pub padding: f32,
    /// From an item's edge to its label (and the separator's ends).
    pub item_padding: f32,
    pub min_width: f32,
    pub max_width: f32,
    /// Each separator's height.
    pub separator_height: f32,
    pub separator_thickness: f32,
    pub separator_color: [f32; 4],
    /// `None`: the theme's `text_color`.
    pub text_color: Option<[f32; 4]>,
    /// `None`: the theme's `muted_text_color`.
    pub heading_color: Option<[f32; 4]>,
    /// `None`: the theme's `accent_color`.
    pub check_color: Option<[f32; 4]>,
    /// `None`: the theme's `default_font_size`.
    pub font_size: Option<f32>,
    /// Under the highlighted action.
    pub highlight: [f32; 4],
    /// The check column, when any item has a check state.
    pub check_width: f32,
    /// The side of the check mark's square.
    pub check_size: f32,
    pub check_thickness: f32,
    /// A disabled action's alpha is multiplied by this.
    pub disabled_alpha: f32,
    /// Alpha 0: no shadow.
    pub shadow: [f32; 4],
    /// How far down the shadow is drawn.
    pub shadow_offset: f32,
    /// From the anchor to the menu.
    pub offset: f32,
    /// The least distance from the menu to the viewport's edges.
    pub margin: f32,
    /// Between a row's columns of cells.
    pub column_gap: f32,
    /// A dim cell's colour. `None`: the theme's `muted_text_color`.
    pub dim_text_color: Option<[f32; 4]>,
}

impl Default for PopupMenuTheme {
    fn default() -> Self {
        Self {
            bg: [0.08, 0.11, 0.16, 0.98],
            border: [0.18, 0.65, 0.92, 0.35],
            border_thickness: 1.0,
            corner_radius: 6.0,
            item_height: 40.0,
            padding: 6.0,
            item_padding: 14.0,
            min_width: 160.0,
            max_width: 320.0,
            separator_height: 9.0,
            separator_thickness: 1.0,
            separator_color: [0.30, 0.36, 0.44, 0.60],
            text_color: None,
            heading_color: None,
            check_color: None,
            font_size: None,
            highlight: [0.18, 0.65, 0.92, 0.20],
            check_width: 24.0,
            check_size: 12.0,
            check_thickness: 1.6,
            disabled_alpha: 0.4,
            shadow: [0.0, 0.0, 0.0, 0.35],
            shadow_offset: 3.0,
            offset: 4.0,
            margin: 8.0,
            column_gap: 12.0,
            dim_text_color: None,
        }
    }
}

/// One menu's overrides of its theme (`None` takes the theme's value) and
/// its own settings.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuOptions {
    pub bg: Option<[f32; 4]>,
    pub border: Option<[f32; 4]>,
    pub border_thickness: Option<f32>,
    pub corner_radius: Option<f32>,
    pub item_height: Option<f32>,
    pub padding: Option<f32>,
    pub item_padding: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub separator_height: Option<f32>,
    pub separator_thickness: Option<f32>,
    pub separator_color: Option<[f32; 4]>,
    pub text_color: Option<[f32; 4]>,
    pub heading_color: Option<[f32; 4]>,
    pub check_color: Option<[f32; 4]>,
    pub font_size: Option<f32>,
    pub highlight: Option<[f32; 4]>,
    pub check_width: Option<f32>,
    pub check_size: Option<f32>,
    pub check_thickness: Option<f32>,
    pub disabled_alpha: Option<f32>,
    pub shadow: Option<[f32; 4]>,
    pub shadow_offset: Option<f32>,
    pub offset: Option<f32>,
    pub margin: Option<f32>,
    pub column_gap: Option<f32>,
    pub dim_text_color: Option<[f32; 4]>,
    /// Which user actions dismiss it (default: outside tap, Escape, Back).
    pub dismiss: DismissOn,
}

macro_rules! setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        #[must_use]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl PopupMenuOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    setter!(bg: [f32; 4]);
    setter!(border: [f32; 4]);
    setter!(border_thickness: f32);
    setter!(corner_radius: f32);
    setter!(item_height: f32);
    setter!(padding: f32);
    setter!(item_padding: f32);
    setter!(min_width: f32);
    setter!(max_width: f32);
    setter!(separator_height: f32);
    setter!(separator_thickness: f32);
    setter!(separator_color: [f32; 4]);
    setter!(text_color: [f32; 4]);
    setter!(heading_color: [f32; 4]);
    setter!(check_color: [f32; 4]);
    setter!(font_size: f32);
    setter!(highlight: [f32; 4]);
    setter!(check_width: f32);
    setter!(check_size: f32);
    setter!(check_thickness: f32);
    setter!(disabled_alpha: f32);
    setter!(shadow: [f32; 4]);
    setter!(shadow_offset: f32);
    setter!(offset: f32);
    setter!(margin: f32);
    setter!(column_gap: f32);
    setter!(dim_text_color: [f32; 4]);

    /// Which user actions dismiss it.
    #[must_use]
    pub fn dismiss(mut self, dismiss: DismissOn) -> Self {
        self.dismiss = dismiss;
        self
    }

    /// Each `Some` wins; each `None` takes the theme's value.
    pub fn with_theme(&self, theme: &PopupMenuTheme) -> PopupMenuResolved {
        PopupMenuResolved {
            bg: self.bg.unwrap_or(theme.bg),
            border: self.border.unwrap_or(theme.border),
            border_thickness: self.border_thickness.unwrap_or(theme.border_thickness),
            corner_radius: self.corner_radius.unwrap_or(theme.corner_radius),
            item_height: self.item_height.unwrap_or(theme.item_height),
            padding: self.padding.unwrap_or(theme.padding),
            item_padding: self.item_padding.unwrap_or(theme.item_padding),
            min_width: self.min_width.unwrap_or(theme.min_width),
            max_width: self.max_width.unwrap_or(theme.max_width),
            separator_height: self.separator_height.unwrap_or(theme.separator_height),
            separator_thickness: self
                .separator_thickness
                .unwrap_or(theme.separator_thickness),
            separator_color: self.separator_color.unwrap_or(theme.separator_color),
            text_color: self.text_color.or(theme.text_color),
            heading_color: self.heading_color.or(theme.heading_color),
            check_color: self.check_color.or(theme.check_color),
            font_size: self.font_size.or(theme.font_size),
            highlight: self.highlight.unwrap_or(theme.highlight),
            check_width: self.check_width.unwrap_or(theme.check_width),
            check_size: self.check_size.unwrap_or(theme.check_size),
            check_thickness: self.check_thickness.unwrap_or(theme.check_thickness),
            disabled_alpha: self.disabled_alpha.unwrap_or(theme.disabled_alpha),
            shadow: self.shadow.unwrap_or(theme.shadow),
            shadow_offset: self.shadow_offset.unwrap_or(theme.shadow_offset),
            offset: self.offset.unwrap_or(theme.offset),
            margin: self.margin.unwrap_or(theme.margin),
            column_gap: self.column_gap.unwrap_or(theme.column_gap),
            dim_text_color: self.dim_text_color.or(theme.dim_text_color),
        }
    }
}

/// A menu's options resolved against its theme: what it reads. Colours and
/// the font size left `None` are the `ControlTheme`'s flat values, read at
/// draw.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuResolved {
    pub bg: [f32; 4],
    pub border: [f32; 4],
    pub border_thickness: f32,
    pub corner_radius: f32,
    pub item_height: f32,
    pub padding: f32,
    pub item_padding: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub separator_height: f32,
    pub separator_thickness: f32,
    pub separator_color: [f32; 4],
    pub text_color: Option<[f32; 4]>,
    pub heading_color: Option<[f32; 4]>,
    pub check_color: Option<[f32; 4]>,
    pub font_size: Option<f32>,
    pub highlight: [f32; 4],
    pub check_width: f32,
    pub check_size: f32,
    pub check_thickness: f32,
    pub disabled_alpha: f32,
    pub shadow: [f32; 4],
    pub shadow_offset: f32,
    pub offset: f32,
    pub margin: f32,
    pub column_gap: f32,
    /// `None`: the theme's `muted_text_color`, read at draw.
    pub dim_text_color: Option<[f32; 4]>,
}

/// A popup menu of actions. The app keeps one, attaches it to its router's
/// modal stack with [`Router::attach_popup_menu`], and opens it with
/// [`PopupMenuState::open`].
#[derive(Clone, Debug)]
pub struct PopupMenu {
    /// Shared state: open, anchor, items, events.
    pub state: Arc<PopupMenuState>,
    /// The viewport the menu stays inside.
    pub rect: Rect,
    /// Theme overrides and settings.
    pub options: PopupMenuOptions,
}

fn rounded(radius: f32) -> SdfKind {
    if radius > 0.0 {
        SdfKind::RoundedBox { radius }
    } else {
        SdfKind::Box
    }
}

fn with_alpha(mut color: [f32; 4], by: f32) -> [f32; 4] {
    color[3] *= by;
    color
}

/// A check mark: two segments in a square of side `s` centred on `c`.
fn draw_check(list: &mut DrawList, c: [f32; 2], s: f32, t: f32, col: [f32; 4]) {
    let at = |dx: f32, dy: f32| [c[0] + dx * s, c[1] + dy * s];
    let knee = at(-0.10, 0.30);
    list.push_line(at(-0.38, 0.02), knee, t, col);
    list.push_line(knee, at(0.38, -0.30), t, col);
}

/// Whether any row reserves the check column: one with a check state, or a
/// toggle (whose `None` is off, so a flip cannot change the width).
fn reserves_check(rows: &[PopupRow]) -> bool {
    rows.iter()
        .any(|r| r.item.checked.is_some() || r.item.kind == PopupItemKind::Toggle)
}

/// The shared columns' kinds and natural widths (plan decisions 8 and 9):
/// column `k` is the `k`th cell of every row with cells.
fn natural_columns(
    rows: &[PopupRow],
    ctx: &DrawContext,
    font_size: f32,
) -> Vec<(PopupCellWidth, f32)> {
    let n = rows
        .iter()
        .map(|r| r.drawn_cells().len())
        .max()
        .unwrap_or(0);
    (0..n)
        .map(|k| {
            let cells = rows.iter().filter_map(|r| r.drawn_cells().get(k));
            let mut fixed: Option<f32> = None;
            let mut flex = false;
            let mut widest = 0.0_f32;
            for cell in cells {
                match cell.width {
                    PopupCellWidth::Fixed(w) => fixed = Some(fixed.map_or(w, |f| f.max(w))),
                    PopupCellWidth::Flex => flex = true,
                    PopupCellWidth::Auto => {}
                }
                widest = widest.max(ctx.text_advance(&cell.text, font_size * cell.used_scale()));
            }
            match fixed {
                Some(w) => (PopupCellWidth::Fixed(w), w.max(0.0)),
                None if flex => (PopupCellWidth::Flex, widest),
                None => (PopupCellWidth::Auto, widest),
            }
        })
        .collect()
}

/// The columns' widths in `room` px: extra room to the `Flex` columns
/// equally (else the last); too little taken from the `Flex` columns in
/// proportion to their natural widths, then from the last column back.
fn column_widths(natural: &[(PopupCellWidth, f32)], gap: f32, room: f32) -> Vec<f32> {
    let mut w: Vec<f32> = natural.iter().map(|&(_, n)| n).collect();
    if w.is_empty() {
        return w;
    }
    let content = w.iter().sum::<f32>() + gap * (w.len() - 1) as f32;
    let flex: Vec<usize> = natural
        .iter()
        .enumerate()
        .filter(|(_, (kind, _))| *kind == PopupCellWidth::Flex)
        .map(|(k, _)| k)
        .collect();
    if content <= room {
        let extra = room - content;
        if flex.is_empty() {
            *w.last_mut().unwrap() += extra;
        } else {
            let share = extra / flex.len() as f32;
            for &k in &flex {
                w[k] += share;
            }
        }
        return w;
    }
    let mut excess = content - room;
    let flex_total: f32 = flex.iter().map(|&k| w[k]).sum();
    if flex_total > 0.0 {
        let take = excess.min(flex_total);
        for &k in &flex {
            w[k] = (w[k] - take * w[k] / flex_total).max(0.0);
        }
        excess -= take;
    }
    for k in (0..w.len()).rev() {
        if excess <= 0.0 {
            break;
        }
        let take = excess.min(w[k]);
        w[k] -= take;
        excess -= take;
    }
    w
}

/// One cell as drawn: its row, cut text, pen, size and colour.
#[derive(Clone, Debug, PartialEq)]
struct CellRun {
    row: usize,
    text: String,
    pen: [f32; 2],
    size: f32,
    color: [f32; 4],
}

impl PopupMenu {
    /// A closed menu that stays inside `viewport`.
    pub fn new(viewport: Rect) -> Self {
        Self {
            state: Arc::new(PopupMenuState::new()),
            rect: viewport,
            options: PopupMenuOptions::default(),
        }
    }

    /// Use an existing shared state.
    #[must_use]
    pub fn with_state(mut self, state: Arc<PopupMenuState>) -> Self {
        self.state = state;
        self
    }

    /// Replace the options.
    #[must_use]
    pub fn with_options(mut self, options: PopupMenuOptions) -> Self {
        self.options = options;
        self
    }

    /// The options resolved against `theme.popup_menu`.
    pub fn resolved(&self, theme: &ControlTheme) -> PopupMenuResolved {
        self.options.with_theme(&theme.popup_menu)
    }

    /// Where the menu and its items go for the state's anchor and items,
    /// measured with `ctx` (plan decision 6).
    pub fn layout(&self, ctx: &DrawContext) -> PopupMenuLayout {
        let r = self.resolved(&ctx.theme);
        let font_size = ctx.font_size(r.font_size);
        let (anchor, rows) = {
            let inner = self.state.lock();
            (inner.anchor, inner.rows.clone())
        };
        let vp = self.rect;
        let label_w = rows
            .iter()
            .filter(|r| r.item.kind != PopupItemKind::Separator && r.cells.is_empty())
            .map(|r| ctx.text_advance(&r.item.label, font_size))
            .fold(0.0_f32, f32::max);
        let natural = natural_columns(&rows, ctx, font_size);
        let cells_w = if natural.is_empty() {
            0.0
        } else {
            natural.iter().map(|&(_, n)| n).sum::<f32>() + r.column_gap * (natural.len() - 1) as f32
        };
        let check_w = if reserves_check(&rows) {
            r.check_width
        } else {
            0.0
        };
        let content_w = label_w.max(cells_w) + 2.0 * r.item_padding + check_w + 2.0 * r.padding;
        let w = content_w
            .max(r.min_width)
            .min(r.max_width)
            .min((vp.w - 2.0 * r.margin).max(0.0));
        let heights: Vec<f32> = rows
            .iter()
            .map(|row| match row.item.kind {
                PopupItemKind::Separator => r.separator_height,
                _ => r.item_height,
            })
            .collect();
        let h = 2.0 * r.padding + heights.iter().sum::<f32>();

        let x = (anchor.right() - w)
            .min(vp.right() - r.margin - w)
            .max(vp.x + r.margin);
        let below_y = anchor.bottom() + r.offset;
        let above_y = anchor.y - r.offset - h;
        let room_below = vp.bottom() - r.margin - below_y;
        let room_above = anchor.y - r.offset - (vp.y + r.margin);
        let (y, above) = if h <= room_below {
            (below_y, false)
        } else if h <= room_above {
            (above_y, true)
        } else if room_above > room_below {
            (above_y.max(vp.y + r.margin), true)
        } else {
            (
                below_y.min(vp.bottom() - r.margin - h).max(vp.y + r.margin),
                false,
            )
        };
        let frame = Rect::new(x, y, w, h);
        let mut top = y + r.padding;
        let inner_w = (w - 2.0 * r.padding).max(0.0);
        let rects = heights
            .iter()
            .map(|&ih| {
                let rect = Rect::new(x + r.padding, top, inner_w, ih);
                top += ih;
                rect
            })
            .collect();
        let room = (inner_w - 2.0 * r.item_padding - check_w).max(0.0);
        let x0 = x + r.padding + r.item_padding + check_w;
        let mut cx = x0;
        let columns = column_widths(&natural, r.column_gap, room)
            .into_iter()
            .map(|cw| {
                let col = [cx, cw];
                cx += cw + r.column_gap;
                col
            })
            .collect();
        PopupMenuLayout {
            frame,
            items: rects,
            above,
            columns,
        }
    }

    /// Where each row's cells are drawn, cut to their columns, and in what
    /// colour (plan decision 10).
    fn cell_runs(
        &self,
        ctx: &DrawContext,
        layout: &PopupMenuLayout,
        rows: &[PopupRow],
    ) -> Vec<CellRun> {
        let r = self.resolved(&ctx.theme);
        let font_size = ctx.font_size(r.font_size);
        let text_col = ctx.text_color(r.text_color);
        let heading_col = r.heading_color.unwrap_or(ctx.theme.muted_text_color);
        let dim_col = r.dim_text_color.unwrap_or(ctx.theme.muted_text_color);
        let mut runs = Vec::new();
        for (i, (row, rect)) in rows.iter().zip(&layout.items).enumerate() {
            let alpha = if row.item.is_dimmed() {
                r.disabled_alpha
            } else {
                1.0
            };
            let base = if row.item.kind == PopupItemKind::Heading {
                heading_col
            } else {
                text_col
            };
            for (cell, &[x, w]) in row.drawn_cells().iter().zip(&layout.columns) {
                let size = font_size * cell.used_scale();
                let measure = |t: &str| ctx.text_advance(t, size);
                let text = cut_to_fit(&cell.text, w, measure);
                let a = measure(&text);
                if a > w {
                    // Narrower than "…" itself: nothing is drawn.
                    continue;
                }
                let px = match cell.align {
                    TextAlign::Left => x,
                    TextAlign::Center => x + (w - a) * 0.5,
                    TextAlign::Right => x + w - a,
                };
                let py = rect.y + (rect.h - size * LINE_BOX_RATIO) * 0.5;
                let col = if cell.dim { dim_col } else { base };
                runs.push(CellRun {
                    row: i,
                    text,
                    pen: [px, py],
                    size,
                    color: with_alpha(col, alpha),
                });
            }
        }
        runs
    }

    /// The last drawn layout, or one from the default theme before a draw.
    fn current_layout(&self) -> PopupMenuLayout {
        self.state
            .layout()
            .unwrap_or_else(|| self.layout(&DrawContext::default()))
    }

    /// The item under `pos` (any kind), by the last drawn layout.
    pub fn item_at(&self, pos: [f32; 2]) -> Option<usize> {
        self.current_layout()
            .items
            .iter()
            .position(|r| r.contains(pos))
    }

    /// The enabled action under `pos`.
    fn choosable_at(&self, pos: [f32; 2]) -> Option<usize> {
        self.item_at(pos)
            .filter(|&i| self.state.lock().choosable(i))
    }

    fn chosen(&self, index: usize) -> Option<ControlAction> {
        self.state
            .activate(index)
            .then_some(ControlAction::Activated {
                part: ControlPart::Custom(index as u32),
            })
    }
}

impl DrawControl for PopupMenu {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let r = self.resolved(&ctx.theme);
        let layout = self.layout(ctx);
        self.state.set_layout(layout.clone());
        let (rows, highlight) = {
            let inner = self.state.lock();
            (inner.rows.clone(), inner.highlight)
        };
        let frame = layout.frame;
        let font_size = ctx.font_size(r.font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let text_col = ctx.text_color(r.text_color);
        let heading_col = r.heading_color.unwrap_or(ctx.theme.muted_text_color);
        let check_col = r.check_color.unwrap_or(ctx.theme.accent_color);
        let check_w = if reserves_check(&rows) {
            r.check_width
        } else {
            0.0
        };
        let cell_runs = self.cell_runs(ctx, &layout, &rows);

        if r.shadow[3] > 0.0 {
            list.push(SdfInstance {
                kind: rounded(r.corner_radius),
                position: [frame.x, frame.y + r.shadow_offset],
                size: frame.size(),
                color: r.shadow,
                anim: 0,
            });
        }
        list.push(SdfInstance {
            kind: rounded(r.corner_radius),
            position: frame.pos(),
            size: frame.size(),
            color: r.bg,
            anim: 0,
        });
        if r.border_thickness > 0.0 {
            list.push(SdfInstance {
                kind: SdfKind::Outline {
                    radius: r.corner_radius,
                    thickness: r.border_thickness,
                },
                position: frame.pos(),
                size: frame.size(),
                color: r.border,
                anim: 0,
            });
        }

        for (i, (row, rect)) in rows.iter().zip(&layout.items).enumerate() {
            let item = &row.item;
            if item.kind == PopupItemKind::Separator {
                let y = rect.y + rect.h * 0.5;
                list.push_line(
                    [rect.x + r.item_padding, y],
                    [rect.right() - r.item_padding, y],
                    r.separator_thickness,
                    r.separator_color,
                );
                continue;
            }
            let alpha = if item.is_dimmed() {
                r.disabled_alpha
            } else {
                1.0
            };
            if highlight == Some(i) && item.is_choosable() {
                list.push(SdfInstance {
                    kind: rounded((r.corner_radius - r.padding * 0.5).max(0.0)),
                    position: rect.pos(),
                    size: rect.size(),
                    color: r.highlight,
                    anim: 0,
                });
            }
            if item.checked == Some(true) {
                let c = [
                    rect.x + r.item_padding * 0.5 + check_w * 0.5,
                    rect.center()[1],
                ];
                draw_check(
                    list,
                    c,
                    r.check_size,
                    r.check_thickness,
                    with_alpha(check_col, alpha),
                );
            }
            if !row.cells.is_empty() {
                for run in cell_runs.iter().filter(|c| c.row == i) {
                    ctx.draw_text(list, &run.text, run.pen, run.size, run.color);
                }
                continue;
            }
            let col = if item.kind == PopupItemKind::Heading {
                heading_col
            } else {
                text_col
            };
            let pen = [
                rect.x + r.item_padding + check_w,
                rect.y + (rect.h - line_box_h) * 0.5,
            ];
            ctx.draw_text(list, &item.label, pen, font_size, with_alpha(col, alpha));
        }
    }
}

impl HostedControl for PopupMenu {
    fn bounds(&self) -> Rect {
        self.current_layout().frame
    }

    /// The menu's frame: an item as `ControlPart::Custom(index)`, the rest
    /// as `Body`.
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        let layout = self.current_layout();
        if !layout.frame.contains(pos) {
            return None;
        }
        let part = layout
            .items
            .iter()
            .position(|r| r.contains(pos))
            .map_or(ControlPart::Body, |i| ControlPart::Custom(i as u32));
        Some((part, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        match *event {
            MouseEvent::Move { pos } => {
                self.state.set_highlight(self.choosable_at(pos));
                None
            }
            MouseEvent::Drag { current_pos, .. } => {
                self.state.set_highlight(self.choosable_at(current_pos));
                None
            }
            MouseEvent::Down { pos, .. } => {
                let at = self.choosable_at(pos);
                self.state.lock().pressed = at;
                self.state.set_highlight(at);
                None
            }
            MouseEvent::Click { pos, .. } => {
                let at = self.choosable_at(pos);
                let pressed = self.state.lock().pressed.take();
                match at {
                    Some(i) if pressed.is_none_or(|p| p == i) => self.chosen(i),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        if !event.is_down() {
            return None;
        }
        match event.key()? {
            Key::Down => self.state.move_highlight(true),
            Key::Up => self.state.move_highlight(false),
            Key::Home => self.state.highlight_end(false),
            Key::End => self.state.highlight_end(true),
            Key::Enter | Key::Space => {
                let highlight = self.state.highlighted()?;
                return self.chosen(highlight);
            }
            _ => {}
        }
        None
    }

    /// The pointer left the menu: no highlight.
    fn set_hover(&self, hover: Option<ControlPart>) {
        if hover.is_none() {
            self.state.set_highlight(None);
        }
    }

    fn default_flags(&self) -> NodeFlags {
        NodeFlags::NONE
    }
}

/// The menu's frame as last laid out: the layer filter's inside.
struct MenuShape(PopupMenu);

impl LayerShape for MenuShape {
    fn contains(&self, pos: [f32; 2]) -> bool {
        self.0.current_layout().frame.contains(pos)
    }
}

impl Modal for PopupMenu {
    /// No backdrop; dismissed by the options' `dismiss`.
    fn options(&self) -> ModalOptions {
        ModalOptions::default()
            .with_backdrop(None)
            .with_dismiss(self.options.dismiss)
    }

    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(ModalLayerFilter::new(
            MenuShape(self.clone()),
            layer.clone(),
        ))
    }
}

impl Router {
    /// Attach `menu` to the installed modal stack as entry `id`: from now on
    /// its state's `open` pushes that entry and the stack draws the menu as
    /// its layer. Attaching an id again replaces the menu for the next open.
    pub fn attach_popup_menu(
        &mut self,
        id: impl Into<Symbol>,
        menu: PopupMenu,
    ) -> Result<(), ModalError> {
        let id = id.into();
        let (_, stack) = self.modal_stack().ok_or(ModalError::NoStack)?;
        menu.state.bind(Arc::clone(stack.state()), id);
        stack.attach(id, Box::new(move || Box::new(menu.clone())));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Drawing cells (plan 20261028_10, step 3): no shaper draws no glyphs,
    //! so the runs `draw` hands to `DrawContext::draw_text` are checked here.

    use super::*;

    const VIEWPORT: Rect = Rect::new(0.0, 0.0, 400.0, 600.0);
    const ANCHOR: Rect = Rect::new(350.0, 8.0, 42.0, 42.0);
    const LONG: &str = "A much longer plan title that will not fit";

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn plan_row(nn: &str, date: &str, title: &str, status: &str) -> PopupRow {
        PopupItem::action("plan", format!("{nn} {date} {title} {status}")).with_cells(vec![
            PopupCell::new(nn).align(TextAlign::Right).scale(0.8),
            PopupCell::new(date).align(TextAlign::Right).scale(0.8),
            PopupCell::new(title).flex(),
            PopupCell::new(status).dim(true).scale(0.8),
        ])
    }

    fn runs_of(menu: &PopupMenu, ctx: &DrawContext) -> (PopupMenuLayout, Vec<CellRun>) {
        let layout = menu.layout(ctx);
        let rows = menu.state.rows();
        let runs = menu.cell_runs(ctx, &layout, &rows);
        (layout, runs)
    }

    fn step2_menu(options: PopupMenuOptions) -> PopupMenu {
        let menu = PopupMenu::new(VIEWPORT).with_options(options);
        menu.state.open_rows(
            ANCHOR,
            vec![
                plan_row("7", "10/28", "Short", "Draft"),
                plan_row("10", "10/27", LONG, "InImpl"),
            ],
        );
        menu
    }

    #[test]
    fn one_run_per_cell_cut_to_its_column() {
        let ctx = DrawContext::default();
        let (layout, runs) = runs_of(&step2_menu(PopupMenuOptions::default()), &ctx);
        assert_eq!(runs.len(), 8);
        assert!(runs.iter().all(|r| r.text != "7 10/28 Short Draft"));
        let title = &runs[6];
        assert!(title.text.ends_with('\u{2026}'), "{}", title.text);
        let w = layout.columns[2][1];
        assert!(approx(w, 163.92));
        assert!(ctx.text_advance(&title.text, 14.0) <= w);
        assert_eq!(runs[2].text, "Short");
    }

    #[test]
    fn cells_are_aligned_in_their_columns_and_centred_on_the_row() {
        let ctx = DrawContext::default();
        let (layout, runs) = runs_of(&step2_menu(PopupMenuOptions::default()), &ctx);
        let row = layout.items[0];
        // "7", right-aligned in 12.32.
        assert!(approx(runs[0].pen[0], 92.0 + 12.32 - 6.16));
        assert!(approx(runs[0].size, 11.2));
        // "Short", left.
        assert!(approx(runs[2].pen[0], 159.12));
        assert!(approx(
            runs[0].pen[1],
            row.y + (40.0 - 11.2 * LINE_BOX_RATIO) / 2.0
        ));
        assert!(approx(
            runs[2].pen[1],
            row.y + (40.0 - 14.0 * LINE_BOX_RATIO) / 2.0
        ));

        let menu = PopupMenu::new(VIEWPORT).with_options(PopupMenuOptions::new().min_width(200.0));
        menu.state.open_rows(
            ANCHOR,
            vec![
                PopupItem::action("c", "ab")
                    .with_cells(vec![PopupCell::new("ab").align(TextAlign::Center).flex()]),
            ],
        );
        let (layout, runs) = runs_of(&menu, &ctx);
        let [x, w] = layout.columns[0];
        assert!(approx(runs[0].pen[0], x + (w - 15.4) / 2.0));
    }

    #[test]
    fn dim_cells_take_the_dim_colour_and_disabled_rows_the_disabled_alpha() {
        let ctx = DrawContext::default();
        let theme = ControlTheme::default();
        let (_, runs) = runs_of(&step2_menu(PopupMenuOptions::default()), &ctx);
        assert_eq!(runs[3].color, theme.muted_text_color);
        assert_eq!(runs[2].color, theme.text_color);

        let red = [1.0, 0.0, 0.0, 1.0];
        let (_, runs) = runs_of(
            &step2_menu(PopupMenuOptions::new().dim_text_color(red)),
            &ctx,
        );
        assert_eq!(runs[3].color, red);

        let menu = PopupMenu::new(VIEWPORT);
        menu.state.open_rows(
            ANCHOR,
            vec![
                PopupItem::action("x", "x y")
                    .with_enabled(false)
                    .with_cells(vec![PopupCell::new("x"), PopupCell::new("y").dim(true)]),
            ],
        );
        let (_, runs) = runs_of(&menu, &ctx);
        assert!(approx(runs[0].color[3], theme.text_color[3] * 0.4));
        assert!(approx(runs[1].color[3], theme.muted_text_color[3] * 0.4));
    }

    #[test]
    fn heading_cells_take_the_heading_colour_and_separator_cells_are_not_drawn() {
        let ctx = DrawContext::default();
        let theme = ControlTheme::default();
        let mut heading_theme = PopupMenuTheme::default();
        let gold = [0.9, 0.8, 0.1, 1.0];
        heading_theme.heading_color = Some(gold);
        let ctx = ctx.with_theme(ControlTheme {
            popup_menu: heading_theme,
            ..theme
        });
        let menu = PopupMenu::new(VIEWPORT);
        menu.state.open_rows(
            ANCHOR,
            vec![
                PopupItem::heading("NN Title")
                    .with_cells(vec![PopupCell::new("NN"), PopupCell::new("Title")]),
                PopupItem::separator().with_cells(vec![PopupCell::new("gone")]),
            ],
        );
        let (_, runs) = runs_of(&menu, &ctx);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().all(|r| r.row == 0 && r.color == gold));
    }

    #[test]
    fn a_column_narrower_than_the_ellipsis_draws_nothing() {
        let natural = [
            (PopupCellWidth::Auto, 50.0),
            (PopupCellWidth::Fixed(40.0), 40.0),
        ];
        // 12 px of room: the last column goes first, then the one before.
        assert_eq!(column_widths(&natural, 10.0, 12.0), vec![2.0, 0.0]);

        let ctx = DrawContext::default();
        let menu = PopupMenu::new(VIEWPORT).with_options(PopupMenuOptions::new().max_width(60.0));
        menu.state.open_rows(
            ANCHOR,
            vec![
                PopupItem::action("x", "x")
                    .with_cells(vec![PopupCell::new("left"), PopupCell::new("right side")]),
            ],
        );
        let (layout, runs) = runs_of(&menu, &ctx);
        // 60 wide: 20 px of room; "left" (30.8) shrinks to 8 and "right
        // side" to 0, narrower than "…" (7.7): only the first draws.
        assert!(approx(layout.columns[0][1], 8.0));
        assert!(approx(layout.columns[1][1], 0.0));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "\u{2026}");
    }

    #[test]
    fn flex_columns_shrink_in_proportion_then_the_rest_from_the_last() {
        let natural = [
            (PopupCellWidth::Flex, 100.0),
            (PopupCellWidth::Flex, 50.0),
            (PopupCellWidth::Auto, 20.0),
        ];
        let w = column_widths(&natural, 0.0, 140.0);
        assert!(approx(w[0], 80.0) && approx(w[1], 40.0) && approx(w[2], 20.0));
        let w = column_widths(&natural, 0.0, 10.0);
        assert_eq!(w, vec![0.0, 0.0, 10.0]);
        // Extra shared equally by the Flex columns.
        let w = column_widths(&natural, 0.0, 190.0);
        assert!(approx(w[0], 110.0) && approx(w[1], 60.0) && approx(w[2], 20.0));
    }

    #[test]
    fn a_cell_is_one_line_and_its_scale_is_clamped() {
        let ctx = DrawContext::default();
        let menu = PopupMenu::new(VIEWPORT);
        menu.state.open_rows(
            ANCHOR,
            vec![
                PopupItem::action("x", "x")
                    .with_cells(vec![PopupCell::new("one\ntwo").scale(100.0)]),
            ],
        );
        let (_, runs) = runs_of(&menu, &ctx);
        assert_eq!(runs[0].size, 14.0 * 4.0);
        assert!(runs[0].text.starts_with("one") || runs[0].text.starts_with('o'));
        assert!(runs[0].text.ends_with('\u{2026}'));
        assert!(!runs[0].text.contains('\n'));
        assert_eq!(PopupCell::new("a").scale(f32::NAN).used_scale(), 1.0);
        assert_eq!(PopupCell::new("a").scale(0.0).used_scale(), 0.25);
    }
}
