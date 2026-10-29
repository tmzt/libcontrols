//! Visual styling, palettes, and typography metrics for UI controls.

pub use crate::drawer::DrawerTheme;
pub use crate::editable_label::EditableLabelTheme;
pub use crate::popup_menu::PopupMenuTheme;
pub use crate::scrollbar::ScrollbarTheme;

/// Visual theme specifying colors, margins, and geometric styles for UI controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlTheme {
    /// Background color for panels and bounded controls `[R, G, B, A]`.
    pub bg_color: [f32; 4],
    /// Primary foreground / label text color `[R, G, B, A]`.
    pub text_color: [f32; 4],
    /// Secondary / muted text color `[R, G, B, A]`.
    pub muted_text_color: [f32; 4],
    /// Primary accent / highlight color `[R, G, B, A]`.
    pub accent_color: [f32; 4],
    /// Unfilled slider track background `[R, G, B, A]`.
    pub track_bg_color: [f32; 4],
    /// Filled slider track active progress `[R, G, B, A]`.
    pub track_fill_color: [f32; 4],
    /// Slider handle / thumb fill color `[R, G, B, A]`.
    pub thumb_color: [f32; 4],
    /// Slider handle / thumb border outline color `[R, G, B, A]`.
    pub thumb_border_color: [f32; 4],
    /// Arrow chevron glyph/vector stroke color `[R, G, B, A]`.
    pub arrow_color: [f32; 4],
    /// Crisp inner outline color for active glowing ring `[R, G, B, A]`.
    pub active_ring_color: [f32; 4],
    /// Diffuse outer halo bloom color for active glowing ring `[R, G, B, A]`.
    pub active_glow_color: [f32; 4],
    /// Default font size in logical screen pixels.
    pub default_font_size: f32,
    /// Distance field antialiasing pixel range for MSDF text.
    pub px_range: f32,
    /// Default corner radius for rounded rects and control boundaries.
    pub corner_radius: f32,
    /// How scroll panes tell a tap from a drag, fling, overscroll and show
    /// their indicator.
    pub scroll: ScrollTheme,
    /// The defaults every drawer starts from; a drawer's `DrawerOptions`
    /// override them (docs/design/20261028_01_DRAWER_PLAN.md, 11:28 EDT).
    pub drawer: DrawerTheme,
    /// The defaults every editable label starts from; its
    /// `EditableLabelOptions` override them
    /// (docs/design/20261028_05_EDITABLE_LABEL_PLAN.md).
    pub editable_label: EditableLabelTheme,
    /// The defaults every popup menu starts from; its `PopupMenuOptions`
    /// override them (docs/design/20261028_06_POPUP_MENU_PLAN.md).
    pub popup_menu: PopupMenuTheme,
    /// The defaults every scrollbar starts from; its `ScrollbarOptions`
    /// override them (docs/design/20261028_08_SCROLLBAR_PLAN.md).
    pub scrollbar: ScrollbarTheme,
    /// The defaults every Markdown view starts from; its `MarkdownOptions`
    /// override them (docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md). Built
    /// in every configuration, with or without the `markdown` feature.
    pub markdown: MarkdownTheme,
}

/// Scroll pane values (docs/design/20261023_02_SCROLL_PANE_PLAN.md, section
/// 3.9), in the router's pixels and seconds. Input reads the values of the
/// theme the router was last drawn with.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollTheme {
    /// How far a press may move before a pane may take it as a scroll
    /// (Material 3: 8 dp).
    pub touch_slop: f32,
    /// Pixels per wheel notch.
    pub wheel_line: f32,
    /// A fling's speed falls to `e^-fling_decay` of itself each second.
    pub fling_decay: f32,
    /// Slower than this (px/s), a fling stops or does not start.
    pub min_fling: f32,
    /// The fastest fling (px/s).
    pub max_fling: f32,
    /// A finger resting this long (s) before it lifts does not fling.
    pub rest: f32,
    /// The overscroll spring's angular frequency (rad/s), critically damped.
    pub overscroll_spring: f32,
    /// The indicator's thickness.
    pub indicator_width: f32,
    /// The indicator's inset from the viewport's edges.
    pub indicator_inset: f32,
    /// The indicator's shortest length.
    pub indicator_min_length: f32,
    /// The indicator's colour at full visibility `[R, G, B, A]`.
    pub indicator_color: [f32; 4],
    /// How long (s) the indicator stays after the pane stops.
    pub indicator_idle: f32,
    /// How long (s) the indicator takes to fade.
    pub indicator_fade: f32,
}

impl Default for ScrollTheme {
    fn default() -> Self {
        Self {
            touch_slop: 8.0,
            wheel_line: 48.0,
            fling_decay: 3.2,
            min_fling: 30.0,
            max_fling: 8000.0,
            rest: 0.100,
            overscroll_spring: 12.0,
            indicator_width: 4.0,
            indicator_inset: 2.0,
            indicator_min_length: 24.0,
            indicator_color: [0.55, 0.62, 0.70, 0.5],
            indicator_idle: 0.5,
            indicator_fade: 0.150,
        }
    }
}

impl Default for ControlTheme {
    fn default() -> Self {
        Self {
            bg_color: [0.08, 0.10, 0.14, 0.85],
            text_color: [0.92, 0.95, 0.98, 1.0],
            muted_text_color: [0.55, 0.62, 0.70, 1.0],
            accent_color: [0.18, 0.72, 0.98, 1.0],
            track_bg_color: [0.15, 0.18, 0.24, 0.9],
            track_fill_color: [0.18, 0.72, 0.98, 0.95],
            thumb_color: [0.95, 0.98, 1.0, 1.0],
            thumb_border_color: [0.18, 0.72, 0.98, 1.0],
            arrow_color: [0.75, 0.85, 0.95, 1.0],
            active_ring_color: [0.25, 0.88, 1.0, 0.95],
            active_glow_color: [0.15, 0.70, 0.98, 0.35],
            default_font_size: 14.0,
            px_range: 6.0,
            corner_radius: 2.0,
            scroll: ScrollTheme::default(),
            drawer: DrawerTheme::default(),
            editable_label: EditableLabelTheme::default(),
            popup_menu: PopupMenuTheme::default(),
            scrollbar: ScrollbarTheme::default(),
            markdown: MarkdownTheme::default(),
        }
    }
}

impl ControlTheme {
    /// Dark fantasy / cyberpunk cyan theme.
    pub fn dark_fantasy() -> Self {
        Self::default()
    }

    /// Gold / solar royalty theme.
    pub fn solar_gold() -> Self {
        Self {
            accent_color: [0.98, 0.78, 0.22, 1.0],
            track_fill_color: [0.95, 0.72, 0.18, 0.95],
            thumb_border_color: [0.98, 0.82, 0.30, 1.0],
            arrow_color: [0.98, 0.90, 0.60, 1.0],
            active_ring_color: [1.0, 0.85, 0.30, 0.95],
            active_glow_color: [0.95, 0.70, 0.15, 0.35],
            ..Self::default()
        }
    }
}

/// The defaults every Markdown view starts from (spec 8.5;
/// docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md, decision 8): flankr's
/// Markdown numbers. Plain data, built with or without the `markdown`
/// feature, so `ControlTheme`'s shape does not depend on features.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MarkdownTheme {
    /// Prose (`None`: the theme's `text_color`).
    pub text_color: Option<[f32; 4]>,
    /// Links and their underlines (`None`: the theme's `accent_color`).
    pub link_color: Option<[f32; 4]>,
    /// Block quotes (`None`: the theme's `muted_text_color`).
    pub dim_color: Option<[f32; 4]>,
    /// Code spans and blocks (`None`: the text colour).
    pub code_color: Option<[f32; 4]>,
    /// The type size (`None`: the theme's `default_font_size`).
    pub font_size: Option<f32>,
    /// A line's height, times the size.
    pub line_height: f32,
    /// The space before a paragraph, heading, quote or list, in lines.
    pub block_gap: f32,
    /// The space before a list item after its list's first, in lines.
    pub item_gap: f32,
    /// A block quote's indent a level, as this many spaces' width.
    pub quote_indent_chars: u32,
    /// A link's underline, times the size (at least 1 px).
    pub underline_thickness: f32,
    /// Where the underline's bottom is, as a share of the line's height.
    pub underline_at: f32,
    /// How far a link's tap area reaches above its line, in lines.
    pub link_tap_above: f32,
    /// A link's tap area's height, in lines.
    pub link_tap_height: f32,
    /// A code character's advance without faces, times the size.
    pub code_cell_em: f32,
}

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            text_color: None,
            link_color: None,
            dim_color: None,
            code_color: None,
            font_size: None,
            line_height: 1.35,
            block_gap: 0.5,
            item_gap: 0.15,
            quote_indent_chars: 4,
            underline_thickness: 0.07,
            underline_at: 0.92,
            link_tap_above: 0.15,
            link_tap_height: 1.3,
            code_cell_em: 0.6,
        }
    }
}

/// One Markdown view's overrides of its theme (`None` takes the theme's
/// value) and its own settings.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct MarkdownOptions {
    pub text_color: Option<[f32; 4]>,
    pub link_color: Option<[f32; 4]>,
    pub dim_color: Option<[f32; 4]>,
    pub code_color: Option<[f32; 4]>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub block_gap: Option<f32>,
    pub item_gap: Option<f32>,
    pub quote_indent_chars: Option<u32>,
    pub underline_thickness: Option<f32>,
    pub underline_at: Option<f32>,
    pub link_tap_above: Option<f32>,
    pub link_tap_height: Option<f32>,
    pub code_cell_em: Option<f32>,
    /// Per instance: each line centred in the view's width, at every
    /// layout (`false` unless set).
    pub centered: bool,
}

macro_rules! md_setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        #[must_use]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl MarkdownOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    md_setter!(text_color: [f32; 4]);
    md_setter!(link_color: [f32; 4]);
    md_setter!(dim_color: [f32; 4]);
    md_setter!(code_color: [f32; 4]);
    md_setter!(font_size: f32);
    md_setter!(line_height: f32);
    md_setter!(block_gap: f32);
    md_setter!(item_gap: f32);
    md_setter!(quote_indent_chars: u32);
    md_setter!(underline_thickness: f32);
    md_setter!(underline_at: f32);
    md_setter!(link_tap_above: f32);
    md_setter!(link_tap_height: f32);
    md_setter!(code_cell_em: f32);

    /// Centre each line in the view's width, at every layout.
    #[must_use]
    pub fn centered(mut self, on: bool) -> Self {
        self.centered = on;
        self
    }

    /// Each `Some` wins; each `None` takes the theme's value.
    pub fn with_theme(&self, theme: &MarkdownTheme) -> MarkdownResolved {
        MarkdownResolved {
            text_color: self.text_color.or(theme.text_color),
            link_color: self.link_color.or(theme.link_color),
            dim_color: self.dim_color.or(theme.dim_color),
            code_color: self.code_color.or(theme.code_color),
            font_size: self.font_size.or(theme.font_size),
            line_height: self.line_height.unwrap_or(theme.line_height),
            block_gap: self.block_gap.unwrap_or(theme.block_gap),
            item_gap: self.item_gap.unwrap_or(theme.item_gap),
            quote_indent_chars: self.quote_indent_chars.unwrap_or(theme.quote_indent_chars),
            underline_thickness: self
                .underline_thickness
                .unwrap_or(theme.underline_thickness),
            underline_at: self.underline_at.unwrap_or(theme.underline_at),
            link_tap_above: self.link_tap_above.unwrap_or(theme.link_tap_above),
            link_tap_height: self.link_tap_height.unwrap_or(theme.link_tap_height),
            code_cell_em: self.code_cell_em.unwrap_or(theme.code_cell_em),
        }
    }
}

/// A Markdown view's options resolved against its theme: what it reads.
/// The colours and the size left `None` are the `ControlTheme`'s (see
/// [`MarkdownTheme`]), read against it with the methods below.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MarkdownResolved {
    pub text_color: Option<[f32; 4]>,
    pub link_color: Option<[f32; 4]>,
    pub dim_color: Option<[f32; 4]>,
    pub code_color: Option<[f32; 4]>,
    pub font_size: Option<f32>,
    pub line_height: f32,
    pub block_gap: f32,
    pub item_gap: f32,
    pub quote_indent_chars: u32,
    pub underline_thickness: f32,
    pub underline_at: f32,
    pub link_tap_above: f32,
    pub link_tap_height: f32,
    pub code_cell_em: f32,
}

impl Default for MarkdownResolved {
    fn default() -> Self {
        MarkdownOptions::default().with_theme(&MarkdownTheme::default())
    }
}

impl MarkdownResolved {
    /// The type size against `theme`.
    pub fn font_size_in(&self, theme: &ControlTheme) -> f32 {
        self.font_size.unwrap_or(theme.default_font_size)
    }

    /// The prose colour against `theme`.
    pub fn text_color_in(&self, theme: &ControlTheme) -> [f32; 4] {
        self.text_color.unwrap_or(theme.text_color)
    }

    /// The link colour against `theme`.
    pub fn link_color_in(&self, theme: &ControlTheme) -> [f32; 4] {
        self.link_color.unwrap_or(theme.accent_color)
    }

    /// The quote colour against `theme`.
    pub fn dim_color_in(&self, theme: &ControlTheme) -> [f32; 4] {
        self.dim_color.unwrap_or(theme.muted_text_color)
    }

    /// The code colour against `theme`.
    pub fn code_color_in(&self, theme: &ControlTheme) -> [f32; 4] {
        self.code_color.unwrap_or_else(|| self.text_color_in(theme))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_options_resolve_against_the_theme_and_the_defaults_are_the_plans() {
        let theme = MarkdownTheme {
            block_gap: 0.75,
            ..MarkdownTheme::default()
        };
        let red = [1.0, 0.0, 0.0, 1.0];
        let r = MarkdownOptions::new().link_color(red).with_theme(&theme);
        assert_eq!((r.link_color, r.block_gap), (Some(red), 0.75));
        let c = ControlTheme::default();
        assert_eq!(r.link_color_in(&c), red);
        let d = MarkdownResolved::default();
        assert_eq!(d.link_color_in(&c), c.accent_color);
        assert_eq!(d.text_color_in(&c), c.text_color);
        assert_eq!(d.dim_color_in(&c), c.muted_text_color);
        assert_eq!(d.code_color_in(&c), c.text_color);
        assert_eq!(d.font_size_in(&c), c.default_font_size);
        let t = MarkdownTheme::default();
        assert_eq!(
            (
                t.text_color,
                t.link_color,
                t.dim_color,
                t.code_color,
                t.font_size
            ),
            (None, None, None, None, None)
        );
        assert_eq!(
            (t.line_height, t.block_gap, t.item_gap, t.quote_indent_chars),
            (1.35, 0.5, 0.15, 4)
        );
        assert_eq!(
            (
                t.underline_thickness,
                t.underline_at,
                t.link_tap_above,
                t.link_tap_height,
                t.code_cell_em
            ),
            (0.07, 0.92, 0.15, 1.3, 0.6)
        );
        assert!(!MarkdownOptions::default().centered);
        assert!(MarkdownOptions::default().centered(true).centered);
        assert_eq!(c.markdown, MarkdownTheme::default());
    }
}
