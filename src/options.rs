//! Options grid: a packed grid of independent cells reading out as a bitfield.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::radio::{DEFAULT_CELL_GAP, DEFAULT_CELL_WIDTH, NO_CELL};
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::tooltip::{TooltipStyle, draw_tooltip, split_label};
use crate::traits::{DrawContext, DrawControl};

/// Most cells an options grid can carry, one per bit of its `u32`.
pub const MAX_OPTIONS: usize = 32;

/// Default cells per row, after which the grid wraps.
pub const DEFAULT_PER_ROW: usize = 5;
/// Default gap between rows in logical pixels.
pub const DEFAULT_ROW_GAP: f32 = 3.0;

/// Background of a cell that is on.
const ON_BG: [f32; 4] = [0.10, 0.26, 0.30, 0.92];
/// Background of a cell that is off.
const OFF_BG: [f32; 4] = [0.05, 0.09, 0.13, 0.90];
/// Border of a cell that is on.
const ON_BORDER: [f32; 4] = [0.42, 0.88, 0.98, 0.85];
/// Border of a cell that is off.
const OFF_BORDER: [f32; 4] = [0.22, 0.46, 0.58, 0.50];
/// Background of a muted cell that is on.
const MUTED_BG: [f32; 4] = [0.09, 0.17, 0.20, 0.80];
/// Border of a muted cell that is on.
const MUTED_BORDER: [f32; 4] = [0.28, 0.52, 0.60, 0.55];

/// Mutable runtime state for an options grid.
#[derive(Debug)]
pub struct OptionsState {
    /// One bit per cell, bit `i` for the cell at index `i`.
    pub bits: AtomicU32,
    /// Cell the keyboard is on, which the ring is drawn around.
    pub cursor: AtomicUsize,
    /// Whether this control is currently focused/active.
    pub is_active: AtomicBool,
    /// The cell the pointer is over, or [`NO_CELL`] for none.
    pub hovered_index: AtomicUsize,
}

impl OptionsState {
    /// Create a new state with the given bits set and the cursor at the start.
    pub fn new(bits: u32) -> Self {
        Self {
            bits: AtomicU32::new(bits),
            cursor: AtomicUsize::new(0),
            is_active: AtomicBool::new(false),
            hovered_index: AtomicUsize::new(NO_CELL),
        }
    }

    /// Read the whole bitfield.
    #[inline]
    pub fn bits(&self) -> u32 {
        self.bits.load(Ordering::Relaxed)
    }

    /// Replace the whole bitfield.
    #[inline]
    pub fn set_bits(&self, bits: u32) {
        self.bits.store(bits, Ordering::Relaxed);
    }

    /// Read the cursor's cell index.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor.load(Ordering::Relaxed)
    }

    /// Move the cursor.
    #[inline]
    pub fn set_cursor(&self, index: usize) {
        self.cursor.store(index, Ordering::Relaxed);
    }

    /// Check if active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set active/focused state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }

    /// The cell the pointer is over, if it is over one.
    #[inline]
    pub fn hovered_cell(&self) -> Option<usize> {
        match self.hovered_index.load(Ordering::Relaxed) {
            NO_CELL => None,
            index => Some(index),
        }
    }

    /// Set the cell the pointer is over.
    #[inline]
    pub fn set_hovered_cell(&self, index: Option<usize>) {
        self.hovered_index
            .store(index.unwrap_or(NO_CELL), Ordering::Relaxed);
    }
}

/// A grid of independent cells, each one a bit of a `u32`.
#[derive(Clone, Debug)]
pub struct Options {
    /// Shared mutable control state.
    pub state: Arc<OptionsState>,
    /// Static slice of cell labels, one per bit.
    pub labels: &'static [&'static str],
    /// Bounding rectangle for the grid.
    pub rect: Rect,
    /// Width of one cell in logical pixels.
    pub cell_width: f32,
    /// Gap between adjacent cells in a row, in logical pixels.
    pub cell_gap: f32,
    /// Gap between rows in logical pixels.
    pub row_gap: f32,
    /// Cells per row, after which the grid wraps.
    pub per_row: usize,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<[f32; 4]>,
    /// Background fill of a cell that is on.
    pub on_bg: [f32; 4],
    /// Background fill of a cell that is off.
    pub off_bg: [f32; 4],
    /// Border outline of a cell that is on.
    pub on_border: [f32; 4],
    /// Border outline of a cell that is off.
    pub off_border: [f32; 4],
    /// Cells drawn at reduced emphasis when on, one bit each.
    pub muted: u32,
    /// Background fill of a muted cell that is on.
    pub muted_bg: [f32; 4],
    /// Border outline of a muted cell that is on.
    pub muted_border: [f32; 4],
    /// Border thickness in pixels.
    pub border_thickness: f32,
    /// Optional corner radius override.
    pub corner_radius: Option<f32>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
    /// Optional custom tooltip style.
    pub tooltip_style: Option<TooltipStyle>,
    /// Whether a `"XX - Name"` label draws its abbreviation with the name as a tooltip (the
    /// default), or the name itself with no tooltip.
    pub abbreviate: bool,
}

impl Options {
    /// Create a new options grid from a static slice of cell labels.
    pub fn new(labels: &'static [&'static str], rect: Rect) -> Self {
        Self {
            state: Arc::new(OptionsState::new(0)),
            labels,
            rect,
            cell_width: DEFAULT_CELL_WIDTH,
            cell_gap: DEFAULT_CELL_GAP,
            row_gap: DEFAULT_ROW_GAP,
            per_row: DEFAULT_PER_ROW,
            font_size: None,
            text_color: None,
            on_bg: ON_BG,
            off_bg: OFF_BG,
            on_border: ON_BORDER,
            off_border: OFF_BORDER,
            muted: 0,
            muted_bg: MUTED_BG,
            muted_border: MUTED_BORDER,
            border_thickness: 1.0,
            corner_radius: Some(3.0),
            glow_style: None,
            tooltip_style: None,
            abbreviate: true,
        }
    }

    /// Attach a shared `OptionsState`.
    pub fn with_state(mut self, state: Arc<OptionsState>) -> Self {
        self.state = state;
        self
    }

    /// Set the whole bitfield.
    pub fn with_bits(self, bits: u32) -> Self {
        self.set_bits(bits);
        self
    }

    /// Set the cell width and the gap between cells in a row.
    pub fn with_cells(mut self, cell_width: f32, cell_gap: f32) -> Self {
        self.cell_width = cell_width;
        self.cell_gap = cell_gap;
        self
    }

    /// Set how many cells a row holds before the grid wraps, and the gap between rows.
    pub fn with_rows(mut self, per_row: usize, row_gap: f32) -> Self {
        self.per_row = per_row.max(1);
        self.row_gap = row_gap;
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

    /// Set the background and border of the cells that are on.
    pub fn with_on_colors(mut self, bg: [f32; 4], border: [f32; 4]) -> Self {
        self.on_bg = bg;
        self.on_border = border;
        self
    }

    /// Set the background and border of the cells that are off.
    pub fn with_off_colors(mut self, bg: [f32; 4], border: [f32; 4]) -> Self {
        self.off_bg = bg;
        self.off_border = border;
        self
    }

    /// Name the cells to draw at reduced emphasis when on, one bit each.
    pub fn with_muted(mut self, muted: u32) -> Self {
        self.muted = muted;
        self
    }

    /// Set the background and border a muted cell takes when on.
    pub fn with_muted_colors(mut self, bg: [f32; 4], border: [f32; 4]) -> Self {
        self.muted_bg = bg;
        self.muted_border = border;
        self
    }

    /// Whether the cell at `index` is drawn at reduced emphasis when on.
    #[inline]
    pub fn is_muted(&self, index: usize) -> bool {
        self.muted & Self::bit(index) != 0
    }

    /// Set border thickness.
    pub fn with_border_thickness(mut self, thickness: f32) -> Self {
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

    /// Set custom tooltip style.
    pub fn with_tooltip_style(mut self, style: TooltipStyle) -> Self {
        self.tooltip_style = Some(style);
        self
    }

    /// Draw abbreviations with tooltips (`true`, the default) or the full names with none
    /// (`false`).
    pub fn with_abbreviate(mut self, abbreviate: bool) -> Self {
        self.abbreviate = abbreviate;
        self
    }

    /// Obtain a borrowed handle to inspect the grid's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Number of cells the grid draws, which is its labels capped at [`MAX_OPTIONS`].
    #[inline]
    pub fn len(&self) -> usize {
        self.labels.len().min(MAX_OPTIONS)
    }

    /// Whether the grid has no cells at all.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Read the whole bitfield.
    #[inline]
    pub fn bits(&self) -> u32 {
        self.state.bits()
    }

    /// Replace the whole bitfield, dropping any bit without a cell.
    #[inline]
    pub fn set_bits(&self, bits: u32) {
        self.state.set_bits(bits & self.mask());
    }

    /// The bits that have a cell, so a register can be masked to what this grid can actually
    /// show.
    #[inline]
    pub fn mask(&self) -> u32 {
        match self.len() {
            0 => 0,
            n if n >= MAX_OPTIONS => u32::MAX,
            n => (1u32 << n) - 1,
        }
    }

    /// Bit value of the cell at `index`, whether or not it exists.
    #[inline]
    pub fn bit(index: usize) -> u32 {
        if index < MAX_OPTIONS {
            1u32 << index
        } else {
            0
        }
    }

    /// Whether the cell at `index` is on.
    #[inline]
    pub fn is_on(&self, index: usize) -> bool {
        self.bits() & Self::bit(index) != 0
    }

    /// Turn the cell at `index` on or off.
    pub fn set_on(&self, index: usize, on: bool) {
        if index >= self.len() {
            return;
        }
        let bit = Self::bit(index);
        let bits = self.bits();
        self.state
            .set_bits(if on { bits | bit } else { bits & !bit });
    }

    /// Flip the cell at `index`, reporting what it became.
    pub fn toggle(&self, index: usize) -> bool {
        let on = !self.is_on(index);
        self.set_on(index, on);
        on
    }

    /// Read the cursor's cell index, held inside the grid.
    #[inline]
    pub fn cursor(&self) -> usize {
        self.state.cursor().min(self.len().saturating_sub(1))
    }

    /// Move the cursor, clamped to the cells that exist.
    #[inline]
    pub fn set_cursor(&self, index: usize) {
        self.state
            .set_cursor(index.min(self.len().saturating_sub(1)));
    }

    /// Read active focus state.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Label of the cell at `index`, if there is one.
    #[inline]
    pub fn label(&self, index: usize) -> Option<&'static str> {
        if index < self.len() {
            self.labels.get(index).copied()
        } else {
            None
        }
    }

    /// Label of the cell the cursor is on.
    #[inline]
    pub fn cursor_label(&self) -> &'static str {
        self.label(self.cursor()).unwrap_or("")
    }

    /// The text cell `index` draws: its label up to the tooltip's separator when abbreviating,
    /// or the name after it when not.
    #[inline]
    pub fn shown_label(&self, index: usize) -> &'static str {
        self.label(index)
            .map(|l| {
                let (short, name) = split_label(l);
                if self.abbreviate {
                    short
                } else {
                    name.unwrap_or(short)
                }
            })
            .unwrap_or("")
    }

    /// Cell `index`'s tooltip, if its label carries one and the cell is abbreviated.
    #[inline]
    pub fn tooltip(&self, index: usize) -> Option<&'static str> {
        if !self.abbreviate {
            return None;
        }
        self.label(index).and_then(|l| split_label(l).1)
    }

    /// The cell the pointer is over, if it is over one.
    #[inline]
    pub fn hovered_cell(&self) -> Option<usize> {
        self.state.hovered_cell().filter(|i| *i < self.len())
    }

    /// Number of rows the cells wrap onto.
    #[inline]
    pub fn rows(&self) -> usize {
        self.len().div_ceil(self.per_row.max(1))
    }

    /// Height the whole grid occupies, which is taller than `rect.h` whenever the cells wrap
    /// onto more than one row.
    pub fn grid_height(&self) -> f32 {
        let rows = self.rows();
        if rows == 0 {
            0.0
        } else {
            rows as f32 * self.rect.h + (rows - 1) as f32 * self.row_gap
        }
    }

    /// Bounding rectangle of the cell at `index`, whether or not it exists.
    #[inline]
    pub fn cell_rect(&self, index: usize) -> Rect {
        let per_row = self.per_row.max(1);
        let (row, col) = (index / per_row, index % per_row);
        Rect::new(
            self.rect.x + col as f32 * (self.cell_width + self.cell_gap),
            self.rect.y + row as f32 * (self.rect.h + self.row_gap),
            self.cell_width,
            self.rect.h,
        )
    }

    /// Bounding rectangles of every cell, in order.
    pub fn cell_rects(&self) -> Vec<Rect> {
        (0..self.len()).map(|i| self.cell_rect(i)).collect()
    }

    /// Width the packed cells occupy, which may be less than the row's.
    pub fn packed_width(&self) -> f32 {
        let across = self.len().min(self.per_row.max(1));
        if across == 0 {
            0.0
        } else {
            across as f32 * self.cell_width + (across - 1) as f32 * self.cell_gap
        }
    }

    /// Index of the cell under `pos`, if the point is on one.
    pub fn cell_at(&self, pos: [f32; 2]) -> Option<usize> {
        (0..self.len()).find(|&i| self.cell_rect(i).contains(pos))
    }
}

impl DrawControl for Options {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);
        let kind = if rad > 0.0 {
            SdfKind::RoundedBox { radius: rad }
        } else {
            SdfKind::Box
        };
        let line_box_h = font_size * LINE_BOX_RATIO;

        for index in 0..self.len() {
            let cell = self.cell_rect(index);
            let on = self.is_on(index);
            let muted = on && self.is_muted(index);
            let (bg, border) = match (on, muted) {
                (false, _) => (self.off_bg, self.off_border),
                (true, true) => (self.muted_bg, self.muted_border),
                (true, false) => (self.on_bg, self.on_border),
            };

            list.push(SdfInstance {
                kind,
                position: cell.pos(),
                size: cell.size(),
                color: bg,
                anim: 0,
            });

            if self.border_thickness > 0.0 {
                list.push(SdfInstance {
                    kind: SdfKind::Outline {
                        radius: rad,
                        thickness: self.border_thickness,
                    },
                    position: cell.pos(),
                    size: cell.size(),
                    color: border,
                    anim: 0,
                });
            }

            let label = self.shown_label(index);
            let text_col = if on && !muted {
                [1.0, 1.0, 1.0, 1.0]
            } else if muted {
                [0.80, 0.86, 0.88, 1.0]
            } else {
                ctx.text_color(self.text_color)
            };
            let advance = ctx.text_advance(label, font_size);
            let pen_x = cell.x + (cell.w - advance) * 0.5;
            let pen_y = cell.y + (cell.h - line_box_h) * 0.5;
            ctx.draw_text(list, label, [pen_x, pen_y], font_size, text_col);
        }

        if self.is_active() && !self.is_empty() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.cell_rect(self.cursor()), &style, &ctx.theme);
        }
    }

    fn draw_overlay(&self, list: &mut DrawList, ctx: &DrawContext) {
        let Some(index) = self.hovered_cell() else {
            return;
        };
        if let Some(tip) = self.tooltip(index) {
            let style = self.tooltip_style.unwrap_or_default();
            draw_tooltip(list, ctx, self.cell_rect(index), tip, &style);
        }
    }
}

impl HostedControl for Options {
    fn bounds(&self) -> Rect {
        Rect::new(self.rect.x, self.rect.y, self.rect.w, self.grid_height())
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        let index = self.cell_at(pos)?;
        Some((
            ControlPart::Custom(index as u32),
            self.cell_rect(index).normalized_x(pos[0]),
        ))
    }

    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction> {
        match event {
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            } => {
                let ControlPart::Custom(index) = hit_part else {
                    return None;
                };
                let index = index as usize;
                if index >= self.len() {
                    return None;
                }
                self.set_cursor(index);
                self.toggle(index);
                Some(ControlAction::Activated { part: hit_part })
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if !event.is_down() || self.is_empty() {
            return None;
        }
        let per_row = self.per_row.max(1);
        let cursor = self.cursor();
        match event.key() {
            Some(Key::Left) => {
                if cursor > 0 {
                    self.set_cursor(cursor - 1);
                    Some(ControlAction::StepLeft)
                } else {
                    None
                }
            }
            Some(Key::Right) => {
                if cursor + 1 < self.len() {
                    self.set_cursor(cursor + 1);
                    Some(ControlAction::StepRight)
                } else {
                    None
                }
            }
            Some(Key::Up) => {
                if cursor >= per_row {
                    self.set_cursor(cursor - per_row);
                    Some(ControlAction::StepLeft)
                } else {
                    None
                }
            }
            Some(Key::Down) => {
                if cursor + per_row < self.len() {
                    self.set_cursor(cursor + per_row);
                    Some(ControlAction::StepRight)
                } else {
                    None
                }
            }
            Some(Key::Enter | Key::Space) => {
                self.toggle(cursor);
                Some(ControlAction::Activated {
                    part: ControlPart::Custom(cursor as u32),
                })
            }
            _ => None,
        }
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        self.state.set_hovered_cell(match hover {
            Some(ControlPart::Custom(index)) => Some(index as usize),
            _ => None,
        });
    }
}

impl ReadValue<u32> for Options {
    fn read_value(&self) -> u32 {
        self.bits()
    }
}

impl ReadValue<usize> for Options {
    fn read_value(&self) -> usize {
        self.cursor()
    }
}

impl ReadValue<&'static str> for Options {
    fn read_value(&self) -> &'static str {
        self.cursor_label()
    }
}
