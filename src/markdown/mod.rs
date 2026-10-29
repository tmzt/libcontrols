//! Basic Markdown, laid out and drawn by libcontrols
//! (docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md): flankr-markdown's
//! layout and flankr's `MarkdownView`, moved, measured with libcontrols' own
//! faces (no measure callback) and themed by `ControlTheme::markdown`.
//!
//! Basic formatting only. Two cargo features, none on by default:
//!
//! - **`markdown`** builds this module with libcontrols' minimal parser and
//!   no new dependency: paragraphs split at blank lines, bold, italic, bold
//!   italic, code spans, backslash escapes and bare `http(s)://` links.
//!   Block syntax shows as written: a `# Title` line is the text "# Title",
//!   a list line is as written, a fence is its text with its lines joined.
//! - **`cmark`** adds pulldown-cmark (CommonMark, no GFM) for [`parse`]:
//!   headings (bold), block quotes (dim), code blocks, bullet and numbered
//!   lists and `[text](url)` links; HTML literal, images as their alt text
//!   linked to their URL. It also brings [`score_markdown`] and
//!   [`is_markdown`].
//!
//! [`parse_inline`], for one-line text such as a scrollbar's
//! `BookmarkText::Markdown` pills, is the minimal parser in both builds.
//!
//! **Feature unification:** if any crate in a build turns `cmark` on, every
//! [`parse`] and [`MarkdownView`] in that build uses CommonMark. The texts
//! that differ are block syntax the minimal parser shows literally, so this
//! only adds formatting.

#[cfg(feature = "cmark")]
mod cmark;
#[cfg(feature = "cmark")]
mod detect;
mod faces;
mod inline;
mod layout;
mod model;
mod view;

#[cfg(feature = "cmark")]
pub use detect::{DEFAULT_MARKDOWN_THRESHOLD_PCT, MarkdownScore, is_markdown, score_markdown};
pub(crate) use faces::draw_run;
pub use faces::{CodeFace, Face, MarkdownFaces};
pub use inline::parse_inline;
pub use layout::{MarkdownMeasure, lay_out};
pub use model::{MdBlock, MdDocument, MdLaid, MdLine, MdRun, MdStyle};
pub use view::MarkdownView;

use crate::theme::MarkdownTheme;

/// `md` parsed with the theme's default gaps: by the minimal parser, or by
/// pulldown-cmark with the `cmark` feature.
pub fn parse(md: &str) -> MdDocument {
    let t = MarkdownTheme::default();
    parse_with_gaps(md, t.block_gap, t.item_gap)
}

/// `md` parsed with these gaps (in lines) between blocks and list items.
pub(crate) fn parse_with_gaps(md: &str, block_gap: f32, item_gap: f32) -> MdDocument {
    #[cfg(feature = "cmark")]
    {
        cmark::parse_cmark(md, block_gap, item_gap)
    }
    #[cfg(not(feature = "cmark"))]
    {
        let _ = item_gap;
        inline::parse_minimal(md, block_gap)
    }
}
