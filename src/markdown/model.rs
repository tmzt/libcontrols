//! libcontrols' own Markdown model (the plan's decision 3): what both
//! parsers produce and [`lay_out`](super::lay_out) wraps. A caller may build
//! an [`MdDocument`] itself and lay it out without parsing.

/// A run's face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MdStyle {
    Regular,
    Bold,
    Italic,
    /// Drawn in the bold face when faces are given.
    BoldItalic,
    Code,
}

/// Text in one style. A `'\n'` in a block's run is a hard line break.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct MdRun {
    pub text: String,
    pub style: MdStyle,
    /// Its index in the document's `links`, when it is a link's text.
    pub link: Option<usize>,
    /// In a block quote.
    pub dim: bool,
}

impl MdRun {
    /// `text` in `style`, not a link, not dim.
    pub fn new(text: impl Into<String>, style: MdStyle) -> Self {
        Self {
            text: text.into(),
            style,
            link: None,
            dim: false,
        }
    }
}

/// A block before wrapping: a paragraph, a heading, a list item's text, a
/// quote's paragraph or a code block.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdBlock {
    pub runs: Vec<MdRun>,
    /// How many list items it is inside: its text is indented by their
    /// markers' widths ([`MdBlock::outer_markers`]; a level without one is
    /// measured as `"• "`).
    pub list_depth: u8,
    /// The enclosing list items' markers, outermost first, whose widths are
    /// its list indent.
    pub outer_markers: Vec<String>,
    /// How many block quotes it is inside: each indents it by the theme's
    /// `quote_indent_chars` spaces.
    pub quote: u8,
    /// A list item's marker with its space (`"• "`, `"3. "`); the item's
    /// lines hang under its text.
    pub marker: Option<String>,
    /// A code block: spaces are kept and only long lines break.
    pub pre: bool,
    /// The space before it, in lines.
    pub gap: f32,
}

/// Parsed Markdown: its blocks and the links their runs index, in order.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdDocument {
    pub blocks: Vec<MdBlock>,
    pub links: Vec<String>,
}

/// One wrapped line.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdLine {
    pub runs: Vec<MdRun>,
    /// From the left, in pixels as the measure gives them.
    pub indent: f32,
    /// The space above it, in lines: the block gap before a block, the item
    /// gap before a list item, 0 within a block.
    pub gap_before: f32,
}

/// The laid-out text, and the links its runs index, in order.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdLaid {
    pub lines: Vec<MdLine>,
    pub links: Vec<String>,
}

/// `url`'s index in `links`, added when new.
pub(crate) fn link_index(links: &mut Vec<String>, url: &str) -> usize {
    match links.iter().position(|l| l == url) {
        Some(i) => i,
        None => {
            links.push(url.to_string());
            links.len() - 1
        }
    }
}

/// The URL schemes found as bare links, as flankr's daemon finds them.
const SCHEMES: [&str; 2] = ["http://", "https://"];
/// What ends a sentence or wraps a URL, and is not part of it at its end.
const TRAILING: [char; 13] = [
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '`',
];

/// `url` without the sentence or wrapping marks at its end; a `)` that
/// closes a `(` inside it stays.
fn trim_url(mut url: &str) -> &str {
    while let Some(last) = url.chars().next_back().filter(|c| TRAILING.contains(c)) {
        if last == ')' && url.matches('(').count() >= url.matches(')').count() {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    url
}

/// The bare `http(s)://` URLs in `text`, as byte ranges, by the daemon's
/// rules: not after a letter or digit, up to whitespace, without trailing
/// punctuation, and not cut short with "…".
pub(crate) fn bare_urls(text: &str) -> Vec<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    for scheme in SCHEMES {
        let mut from = 0;
        while let Some(at) = lower[from..].find(scheme).map(|i| from + i) {
            from = at + scheme.len();
            if text[..at]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            let len = text[at..]
                .char_indices()
                .find(|&(_, c)| c.is_whitespace())
                .map_or(text.len() - at, |(i, _)| i);
            let url = trim_url(&text[at..at + len]);
            from = at + len;
            if url.len() <= scheme.len() || url.ends_with('\u{2026}') {
                continue;
            }
            found.push((at, at + url.len()));
        }
    }
    found.sort_unstable();
    found
}
