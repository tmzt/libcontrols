//! Radio strip: a packed row of mutually exclusive cells reading out as an index.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::tooltip::{TooltipStyle, draw_tooltip, split_label};
use crate::traits::{DrawContext, DrawControl};

/// Default cell width in logical pixels, matching the panel tab strip.
pub const DEFAULT_CELL_WIDTH: f32 = 30.0;
/// Default gap between cells in logical pixels, matching the panel tab strip.
pub const DEFAULT_CELL_GAP: f32 = 3.0;

/// Background of the cell that is on.
const SELECTED_BG: [f32; 4] = [0.10, 0.26, 0.30, 0.92];
/// Background of a cell that is off.
const UNSELECTED_BG: [f32; 4] = [0.05, 0.09, 0.13, 0.90];
/// Border of the cell that is on.
const SELECTED_BORDER: [f32; 4] = [0.42, 0.88, 0.98, 0.85];
/// Border of a cell that is off.
const UNSELECTED_BORDER: [f32; 4] = [0.22, 0.46, 0.58, 0.50];

/// The hovered index that means the pointer is on no cell.
pub const NO_CELL: usize = usize::MAX;

/// Mutable runtime state for a radio strip.
#[derive(Debug)]
pub struct RadioState {
    /// Currently selected cell index.
    pub selected_index: AtomicUsize,
    /// Whether this control is currently focused/active.
    pub is_active: AtomicBool,
    /// The cell the pointer is over, or [`NO_CELL`] for none.
    pub hovered_index: AtomicUsize,
}

impl RadioState {
    /// Create a new state with the given cell selected.
    pub fn new(selected_index: usize) -> Self {
        Self {
            selected_index: AtomicUsize::new(selected_index),
            is_active: AtomicBool::new(false),
            hovered_index: AtomicUsize::new(NO_CELL),
        }
    }

    /// Read the currently selected cell index.
    #[inline]
    pub fn selected_index(&self) -> usize {
        self.selected_index.load(Ordering::Relaxed)
    }

    /// Set the selected cell index.
    #[inline]
    pub fn set_selected_index(&self, index: usize) {
        self.selected_index.store(index, Ordering::Relaxed);
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

/// A strip of mutually exclusive cells, exactly one of which is on.
#[derive(Clone, Debug)]
pub struct Radio {
    /// Shared mutable control state.
    pub state: Arc<RadioState>,
    /// Static slice of cell labels.
    pub labels: &'static [&'static str],
    /// Bounding rectangle for the strip.
    pub rect: Rect,
    /// Width of one cell in logical pixels.
    pub cell_width: f32,
    /// Gap between adjacent cells in logical pixels.
    pub cell_gap: f32,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<[f32; 4]>,
    /// Background fill of the selected cell.
    pub selected_bg: [f32; 4],
    /// Background fill of an unselected cell.
    pub unselected_bg: [f32; 4],
    /// Border outline of the selected cell.
    pub selected_border: [f32; 4],
    /// Border outline of an unselected cell.
    pub unselected_border: [f32; 4],
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

impl Radio {
    /// Create a new radio strip from a static slice of cell labels.
    pub fn new(labels: &'static [&'static str], rect: Rect) -> Self {
        Self {
            state: Arc::new(RadioState::new(0)),
            labels,
            rect,
            cell_width: DEFAULT_CELL_WIDTH,
            cell_gap: DEFAULT_CELL_GAP,
            font_size: None,
            text_color: None,
            selected_bg: SELECTED_BG,
            unselected_bg: UNSELECTED_BG,
            selected_border: SELECTED_BORDER,
            unselected_border: UNSELECTED_BORDER,
            border_thickness: 1.0,
            corner_radius: Some(3.0),
            glow_style: None,
            tooltip_style: None,
            abbreviate: true,
        }
    }

    /// Attach a shared `RadioState`.
    pub fn with_state(mut self, state: Arc<RadioState>) -> Self {
        self.state = state;
        self
    }

    /// Set the selected cell index, clamped to the available cells.
    pub fn with_selected_index(self, index: usize) -> Self {
        self.set_selected_index(index);
        self
    }

    /// Set the cell width and the gap between cells.
    pub fn with_cells(mut self, cell_width: f32, cell_gap: f32) -> Self {
        self.cell_width = cell_width;
        self.cell_gap = cell_gap;
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

    /// Set the background and border of the cell that is on.
    pub fn with_selected_colors(mut self, bg: [f32; 4], border: [f32; 4]) -> Self {
        self.selected_bg = bg;
        self.selected_border = border;
        self
    }

    /// Set the background and border of the cells that are off.
    pub fn with_unselected_colors(mut self, bg: [f32; 4], border: [f32; 4]) -> Self {
        self.unselected_bg = bg;
        self.unselected_border = border;
        self
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

    /// Obtain a borrowed handle to inspect the strip's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Number of cells.
    #[inline]
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether the strip has no cells at all.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Read the currently selected cell index.
    #[inline]
    pub fn selected_index(&self) -> usize {
        self.state.selected_index()
    }

    /// Set the selected cell index, clamped to the available cells.
    #[inline]
    pub fn set_selected_index(&self, index: usize) {
        let clamped = if self.labels.is_empty() {
            0
        } else {
            index.min(self.labels.len() - 1)
        };
        self.state.set_selected_index(clamped);
    }

    /// Read active focus state.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Label of the cell at `index`, if there is one.
    #[inline]
    pub fn label(&self, index: usize) -> Option<&'static str> {
        self.labels.get(index).copied()
    }

    /// Label of the selected cell.
    #[inline]
    pub fn selected_label(&self) -> &'static str {
        self.label(self.selected_index()).unwrap_or("")
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
        self.state.hovered_cell().filter(|i| *i < self.labels.len())
    }

    /// Whether the selection can step left.
    #[inline]
    pub fn left_enabled(&self) -> bool {
        self.selected_index() > 0
    }

    /// Whether the selection can step right.
    #[inline]
    pub fn right_enabled(&self) -> bool {
        !self.labels.is_empty() && self.selected_index() + 1 < self.labels.len()
    }

    /// Bounding rectangle of the cell at `index`, whether or not it exists.
    #[inline]
    pub fn cell_rect(&self, index: usize) -> Rect {
        Rect::new(
            self.rect.x + index as f32 * (self.cell_width + self.cell_gap),
            self.rect.y,
            self.cell_width,
            self.rect.h,
        )
    }

    /// Bounding rectangles of every cell, in order.
    pub fn cell_rects(&self) -> Vec<Rect> {
        (0..self.labels.len()).map(|i| self.cell_rect(i)).collect()
    }

    /// Width the packed cells occupy, which may be less than the row's.
    pub fn packed_width(&self) -> f32 {
        if self.labels.is_empty() {
            0.0
        } else {
            self.labels.len() as f32 * self.cell_width
                + (self.labels.len() - 1) as f32 * self.cell_gap
        }
    }

    /// Index of the cell under `pos`, if the point is on one.
    pub fn cell_at(&self, pos: [f32; 2]) -> Option<usize> {
        (0..self.labels.len()).find(|&i| self.cell_rect(i).contains(pos))
    }
}

impl DrawControl for Radio {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);
        let kind = if rad > 0.0 {
            SdfKind::RoundedBox { radius: rad }
        } else {
            SdfKind::Box
        };
        let selected = self.selected_index();
        let line_box_h = font_size * LINE_BOX_RATIO;

        for index in 0..self.labels.len() {
            let cell = self.cell_rect(index);
            let on = index == selected;

            list.push(SdfInstance {
                kind,
                position: cell.pos(),
                size: cell.size(),
                color: if on {
                    self.selected_bg
                } else {
                    self.unselected_bg
                },
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
                    color: if on {
                        self.selected_border
                    } else {
                        self.unselected_border
                    },
                    anim: 0,
                });
            }

            let text_col = if on {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                ctx.text_color(self.text_color)
            };
            let label = self.shown_label(index);
            let advance = ctx.text_advance(label, font_size);
            let pen_x = cell.x + (cell.w - advance) * 0.5;
            let pen_y = cell.y + (cell.h - line_box_h) * 0.5;
            ctx.draw_text(list, label, [pen_x, pen_y], font_size, text_col);
        }

        if self.is_active() && selected < self.labels.len() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.cell_rect(selected), &style, &ctx.theme);
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

impl HostedControl for Radio {
    fn bounds(&self) -> Rect {
        self.rect
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
            }
            | MouseEvent::Down {
                button: MouseButton::Left,
                ..
            } => {
                let ControlPart::Custom(index) = hit_part else {
                    return None;
                };
                let index = index as usize;
                if index >= self.labels.len() {
                    return None;
                }
                self.set_selected_index(index);
                Some(ControlAction::Activated { part: hit_part })
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if !event.is_down() {
            return None;
        }
        match event.key() {
            Some(Key::Left | Key::Up) => {
                if self.left_enabled() {
                    self.set_selected_index(self.selected_index() - 1);
                    Some(ControlAction::StepLeft)
                } else {
                    None
                }
            }
            Some(Key::Right | Key::Down) => {
                if self.right_enabled() {
                    self.set_selected_index(self.selected_index() + 1);
                    Some(ControlAction::StepRight)
                } else {
                    None
                }
            }
            Some(Key::Enter | Key::Space) => {
                if self.labels.is_empty() {
                    None
                } else {
                    Some(ControlAction::Activated {
                        part: ControlPart::Custom(self.selected_index() as u32),
                    })
                }
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

impl ReadValue<usize> for Radio {
    fn read_value(&self) -> usize {
        self.selected_index()
    }
}

impl ReadValue<&'static str> for Radio {
    fn read_value(&self) -> &'static str {
        self.selected_label()
    }
}
