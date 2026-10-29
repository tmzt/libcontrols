//! Bookmarks drawn while the thumb is held (the plan's decision 5): pills
//! at their sections' places on the scrollbar's scale, centred across the
//! track, the nearest brighter, crowded ones giving way to the nearest.

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::rect::Rect;
use crate::sections::SectionId;
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

/// Lay the bookmarks out: `marks` are each bookmark's section, top and text;
/// `content` is the scroller's content length; `thumb_centre` the thumb's
/// centre on the screen.
pub(crate) fn layout(
    track: Rect,
    r: &ScrollbarResolved,
    content: f32,
    thumb_centre: f32,
    marks: Vec<(SectionId, f32, String)>,
    measure: impl Fn(&str) -> f32,
) -> Vec<BookmarkPill> {
    let h = r.bookmark_font_size * r.bookmark_line_height + 2.0 * r.bookmark_pad_y;
    let max_text = (track.w - 2.0 * r.bookmark_side_margin - 2.0 * r.bookmark_pad_x).max(0.0);
    let content = content.max(1.0);
    let mut candidates: Vec<(f32, BookmarkPill)> = marks
        .into_iter()
        .map(|(id, top, text)| {
            let y = track.y + (top / content).clamp(0.0, 1.0) * track.h;
            let text = cut_to_fit(&text, max_text, &measure);
            let w = measure(&text) + 2.0 * r.bookmark_pad_x;
            let rect = Rect::new(track.x + (track.w - w) * 0.5, y - h * 0.5, w, h);
            (
                (y - thumb_centre).abs(),
                BookmarkPill {
                    id,
                    rect,
                    text,
                    nearest: false,
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
        ctx.draw_text(list, &pill.text, pen, r.bookmark_font_size, text);
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
}
