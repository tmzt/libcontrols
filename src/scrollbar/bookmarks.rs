//! Bookmarks drawn while the thumb is held (the plan's decision 5): pills
//! at their sections' places on the scrollbar's scale, centred across the
//! track, the nearest brighter, crowded ones giving way to the nearest.
//! With the `markdown` feature a `BookmarkText::Markdown` pill is drawn as
//! inline spans (the plan's decision 6, through the Markdown view plan's
//! `parse_inline`).

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

#[cfg(feature = "markdown")]
use crate::markdown::{MarkdownFaces, MarkdownMeasure, MdRun, MdStyle, draw_run, parse_inline};
use crate::rect::Rect;
use crate::sections::{BookmarkText, SectionId};
#[cfg(feature = "markdown")]
use crate::theme::MarkdownOptions;
use crate::traits::DrawContext;

use super::ScrollbarResolved;

/// One bookmark as drawn this frame.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BookmarkPill {
    /// The section it is on.
    pub id: SectionId,
    /// The pill.
    pub rect: Rect,
    /// The visible text, cut to fit.
    pub text: String,
    /// Whether it is the one nearest the thumb.
    pub nearest: bool,
    /// A Markdown bookmark's spans, cut to fit (`text` is their text).
    #[cfg(feature = "markdown")]
    pub(crate) runs: Option<Vec<MdRun>>,
}

/// How a pill's text is measured: the context's font at the bookmark size;
/// with the `markdown` feature, a Markdown span by the faces when given,
/// else the context's font with a code chip's padding.
pub(crate) struct PillMeasure<'a> {
    ctx: &'a DrawContext<'a>,
    size: f32,
    #[cfg(feature = "markdown")]
    faces: Option<&'a MarkdownFaces>,
    #[cfg(feature = "markdown")]
    code_pad: f32,
}

impl<'a> PillMeasure<'a> {
    pub(crate) fn new(
        ctx: &'a DrawContext<'a>,
        r: &ScrollbarResolved,
        #[cfg(feature = "markdown")] faces: Option<&'a MarkdownFaces>,
    ) -> Self {
        Self {
            ctx,
            size: r.bookmark_font_size,
            #[cfg(feature = "markdown")]
            faces,
            #[cfg(feature = "markdown")]
            code_pad: r.bookmark_code_pad,
        }
    }

    fn plain(&self, text: &str) -> f32 {
        self.ctx.text_advance(text, self.size)
    }

    /// A span's advance: with faces, its face's; without, the context's
    /// font, and a code span its chip's padding either side.
    #[cfg(feature = "markdown")]
    fn run(&self, run: &MdRun) -> f32 {
        match self.faces {
            Some(f) => {
                let theme = MarkdownOptions::default().with_theme(&self.ctx.theme.markdown);
                MarkdownMeasure::new(Some(f), self.size, &theme).width(&run.text, run.style)
            }
            None => {
                let pad = if run.style == MdStyle::Code {
                    2.0 * self.code_pad
                } else {
                    0.0
                };
                self.plain(&run.text) + pad
            }
        }
    }

    #[cfg(feature = "markdown")]
    fn runs(&self, runs: &[MdRun]) -> f32 {
        runs.iter().map(|r| self.run(r)).sum()
    }
}

const ELLIPSIS: char = '\u{2026}';

/// `text` on one line within `max` (as `measure` measures it), cut with "…"
/// when it does not fit or has more lines.
pub(crate) fn cut_to_fit(text: &str, max: f32, measure: impl Fn(&str) -> f32) -> String {
    let (first, more) = match text.split_once('\n') {
        Some((first, _)) => (first.trim_end_matches('\r'), true),
        None => (text, false),
    };
    if !more && measure(first) <= max {
        return first.to_owned();
    }
    let chars: Vec<char> = first.chars().collect();
    let mut n = chars.len();
    loop {
        let mut cut: String = chars[..n].iter().collect();
        let cut_trimmed = cut.trim_end().len();
        cut.truncate(cut_trimmed);
        cut.push(ELLIPSIS);
        if n == 0 || measure(&cut) <= max {
            return cut;
        }
        n -= 1;
    }
}

/// `text`'s first line as inline Markdown spans within `max`, cut with "…"
/// (counting the visible text) when they do not fit or it has more lines.
#[cfg(feature = "markdown")]
fn cut_runs_to_fit(text: &str, max: f32, m: &PillMeasure) -> Vec<MdRun> {
    let (first, more) = match text.split_once('\n') {
        Some((first, _)) => (first.trim_end_matches('\r'), true),
        None => (text, false),
    };
    let runs = parse_inline(first);
    if !more && m.runs(&runs) <= max {
        return runs;
    }
    let chars: Vec<(char, MdStyle)> = runs
        .iter()
        .flat_map(|r| r.text.chars().map(move |c| (c, r.style)))
        .collect();
    let mut n = chars.len();
    loop {
        let mut keep = n;
        while keep > 0 && chars[keep - 1].0.is_whitespace() {
            keep -= 1;
        }
        let mut cut: Vec<MdRun> = Vec::new();
        let pieces = chars[..keep]
            .iter()
            .copied()
            .chain(std::iter::once((ELLIPSIS, MdStyle::Regular)));
        for (c, style) in pieces {
            match cut.last_mut() {
                Some(last) if last.style == style => last.text.push(c),
                _ => cut.push(MdRun::new(c.to_string(), style)),
            }
        }
        if n == 0 || m.runs(&cut) <= max {
            return cut;
        }
        n -= 1;
    }
}

/// Lay the bookmarks out: `marks` are each bookmark's section, top and text;
/// `content` is the scroller's content length; `thumb_centre` the thumb's
/// centre on the screen.
pub(crate) fn layout(
    track: Rect,
    r: &ScrollbarResolved,
    content: f32,
    thumb_centre: f32,
    marks: Vec<(SectionId, f32, BookmarkText)>,
    m: &PillMeasure,
) -> Vec<BookmarkPill> {
    let h = r.bookmark_font_size * r.bookmark_line_height + 2.0 * r.bookmark_pad_y;
    let max_text = (track.w - 2.0 * r.bookmark_side_margin - 2.0 * r.bookmark_pad_x).max(0.0);
    let content = content.max(1.0);
    let mut candidates: Vec<(f32, BookmarkPill)> = marks
        .into_iter()
        .map(|(id, top, mark)| {
            let y = track.y + (top / content).clamp(0.0, 1.0) * track.h;
            #[cfg(feature = "markdown")]
            let (text, w, runs) = match &mark {
                BookmarkText::Markdown(src) => {
                    let runs = cut_runs_to_fit(src, max_text, m);
                    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
                    (text, m.runs(&runs), Some(runs))
                }
                other => {
                    let text = cut_to_fit(other.plain(), max_text, |t| m.plain(t));
                    let w = m.plain(&text);
                    (text, w, None)
                }
            };
            #[cfg(not(feature = "markdown"))]
            let (text, w) = {
                let text = cut_to_fit(mark.plain(), max_text, |t| m.plain(t));
                let w = m.plain(&text);
                (text, w)
            };
            let w = w + 2.0 * r.bookmark_pad_x;
            let rect = Rect::new(track.x + (track.w - w) * 0.5, y - h * 0.5, w, h);
            (
                (y - thumb_centre).abs(),
                BookmarkPill {
                    id,
                    rect,
                    text,
                    nearest: false,
                    #[cfg(feature = "markdown")]
                    runs,
                },
            )
        })
        .collect();
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let gap = r.bookmark_gap;
    let mut shown: Vec<BookmarkPill> = Vec::new();
    for (i, (_, mut pill)) in candidates.into_iter().enumerate() {
        let p = pill.rect;
        let clear = shown
            .iter()
            .all(|o| p.bottom() + gap <= o.rect.y || p.y >= o.rect.bottom() + gap);
        if clear {
            pill.nearest = i == 0;
            shown.push(pill);
        }
    }
    shown.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
    shown
}

/// A pill's background and text colours at the bookmarks' `fade`.
pub(crate) fn pill_colors(
    r: &ScrollbarResolved,
    base: [f32; 4],
    nearest: bool,
    fade: f32,
) -> ([f32; 4], [f32; 4]) {
    let (bg, text) = if nearest {
        (r.bookmark_near_bg_alpha, r.bookmark_near_text_alpha)
    } else {
        (r.bookmark_bg_alpha, r.bookmark_text_alpha)
    };
    let with = |a: f32| [base[0], base[1], base[2], base[3] * a * fade];
    (with(bg), with(text))
}

/// Draw the pills.
pub(crate) fn draw(
    list: &mut DrawList,
    ctx: &DrawContext,
    r: &ScrollbarResolved,
    pills: &[BookmarkPill],
    fade: f32,
    #[cfg(feature = "markdown")] faces: Option<&MarkdownFaces>,
) {
    if fade <= 0.0 {
        return;
    }
    let base = r.bookmark_color.unwrap_or(ctx.theme.text_color);
    for pill in pills {
        let (bg, text) = pill_colors(r, base, pill.nearest, fade);
        let rect = pill.rect;
        list.push(SdfInstance {
            kind: SdfKind::RoundedBox {
                radius: rect.h * 0.5,
            },
            position: rect.pos(),
            size: rect.size(),
            color: bg,
            anim: 0,
        });
        let line = r.bookmark_font_size * r.bookmark_line_height;
        let pen = [rect.x + r.bookmark_pad_x, rect.y + (rect.h - line) * 0.5];
        #[cfg(feature = "markdown")]
        if let Some(runs) = &pill.runs {
            let spans = Spans {
                r,
                base,
                fade,
                bg,
                text,
                line,
                faces,
            };
            spans.draw(list, ctx, runs, pen);
            continue;
        }
        ctx.draw_text(list, &pill.text, pen, r.bookmark_font_size, text);
    }
}

/// How one Markdown pill's spans are drawn.
#[cfg(feature = "markdown")]
struct Spans<'a> {
    r: &'a ScrollbarResolved,
    base: [f32; 4],
    fade: f32,
    /// The pill's background and text colours.
    bg: [f32; 4],
    text: [f32; 4],
    /// The text's line height.
    line: f32,
    faces: Option<&'a MarkdownFaces>,
}

#[cfg(feature = "markdown")]
impl Spans<'_> {
    /// A span's text colour: without faces, bold is told apart by the
    /// nearest pill's text alpha.
    fn color(&self, style: MdStyle) -> [f32; 4] {
        let bold = matches!(style, MdStyle::Bold | MdStyle::BoldItalic);
        if self.faces.is_none() && bold {
            let a = self
                .r
                .bookmark_near_text_alpha
                .max(self.r.bookmark_text_alpha);
            let b = self.base;
            [b[0], b[1], b[2], b[3] * a * self.fade]
        } else {
            self.text
        }
    }

    fn draw(&self, list: &mut DrawList, ctx: &DrawContext, runs: &[MdRun], pen: [f32; 2]) {
        let m = PillMeasure::new(ctx, self.r, self.faces);
        let size = self.r.bookmark_font_size;
        let mut x = pen[0];
        for run in runs {
            let w = m.run(run);
            let mut tx = x;
            if self.faces.is_none() && run.style == MdStyle::Code {
                // A chip at the pill's alpha again, padded either side.
                list.push(SdfInstance {
                    kind: SdfKind::RoundedBox {
                        radius: self.r.bookmark_code_radius,
                    },
                    position: [x, pen[1]],
                    size: [w, self.line],
                    color: self.bg,
                    anim: 0,
                });
                tx += self.r.bookmark_code_pad;
            }
            let color = self.color(run.style);
            draw_run(
                list,
                ctx,
                self.faces,
                &run.text,
                run.style,
                [tx, pen[1]],
                size,
                color,
            );
            x += w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrollbar::{ScrollbarOptions, ScrollbarTheme};

    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn a_long_text_is_cut_with_an_ellipsis() {
        assert_eq!(cut_to_fit("short", 100.0, measure), "short");
        let cut = cut_to_fit("a very long bookmark", 100.0, measure);
        assert!(cut.ends_with(ELLIPSIS) && measure(&cut) <= 100.0, "{cut}");
        assert_eq!(cut_to_fit("one\ntwo", 100.0, measure), "one\u{2026}");
    }

    #[test]
    fn the_nearest_is_brighter() {
        let r = ScrollbarOptions::default().with_theme(&ScrollbarTheme::default());
        let (bg, text) = pill_colors(&r, [1.0, 1.0, 1.0, 1.0], true, 1.0);
        assert_eq!((bg[3], text[3]), (0.20, 0.90));
        let (bg, text) = pill_colors(&r, [1.0, 1.0, 1.0, 1.0], false, 0.5);
        assert_eq!((bg[3], text[3]), (0.05, 0.30));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn without_faces_a_bold_span_is_brighter_than_plain() {
        let r = ScrollbarOptions::default().with_theme(&ScrollbarTheme::default());
        let base = [1.0, 1.0, 1.0, 1.0];
        let (bg, text) = pill_colors(&r, base, false, 1.0);
        let spans = Spans {
            r: &r,
            base,
            fade: 1.0,
            bg,
            text,
            line: 17.55,
            faces: None,
        };
        assert_eq!(spans.color(MdStyle::Regular)[3], 0.60);
        assert_eq!(spans.color(MdStyle::Italic)[3], 0.60);
        assert_eq!(spans.color(MdStyle::Bold)[3], 0.90);
        assert_eq!(spans.color(MdStyle::BoldItalic)[3], 0.90);
    }
}
