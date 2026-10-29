//! The minimal parser (the plan's decision 4), with no dependency:
//! paragraphs split at blank lines, `***bold italic***`, `**bold**` and
//! `__bold__`, `*italic*` and `_italic_` (an `_` inside a word is literal),
//! `` `code` `` with nothing formatted inside, backslash escapes of
//! ``\ * _ ` ``, and bare `http://`/`https://` URLs as links. Everything else,
//! unmatched markers included, is literal text.
//!
//! [`parse_inline`] is the same inline pass for one line with no links, for
//! bookmark pills (decision 6).

use super::model::{MdBlock, MdDocument, MdRun, MdStyle, bare_urls, link_index};

/// `md` as paragraphs with inline formatting and bare links: the
/// `markdown` feature's [`parse`](super::parse) without `cmark`, with the
/// given block gap.
#[cfg_attr(feature = "cmark", allow(dead_code))]
pub(crate) fn parse_minimal(md: &str, block_gap: f32) -> MdDocument {
    let mut doc = MdDocument::default();
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, doc: &mut MdDocument| {
        if para.is_empty() {
            return;
        }
        // Single newlines are soft breaks, drawn as spaces.
        let text = para.join(" ");
        para.clear();
        let runs = inline_runs(&text, Some(&mut doc.links));
        if runs.is_empty() {
            return;
        }
        let gap = if doc.blocks.is_empty() {
            0.0
        } else {
            block_gap
        };
        doc.blocks.push(MdBlock {
            runs,
            gap,
            ..MdBlock::default()
        });
    };
    for line in md.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(&mut para, &mut doc);
        } else {
            para.push(line);
        }
    }
    flush(&mut para, &mut doc);
    doc
}

/// `text` as one line of runs: bold, italic, bold italic and code, escapes
/// taken, newlines as spaces. Bare URLs are plain text, since a pill is not
/// a link. The minimal parser's inline pass in every build (with or without
/// `cmark`), so a bookmark looks the same in both.
pub fn parse_inline(text: &str) -> Vec<MdRun> {
    let text = text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    inline_runs(&text, None)
}

/// One piece of a paragraph before emphasis is resolved.
#[derive(Debug)]
enum Item {
    /// Literal text (escapes taken).
    Text(String),
    /// A code span's content.
    Code(String),
    /// A bare URL.
    Url(String),
    /// A run of `*` or `_`.
    Delim {
        ch: char,
        /// Its markers not yet matched.
        left: usize,
        can_open: bool,
        can_close: bool,
    },
}

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace())
}

/// Is `c` (or the text's edge, `None`) whitespace for flanking?
fn space(c: Option<char>) -> bool {
    c.is_none_or(char::is_whitespace)
}

fn punct(c: Option<char>) -> bool {
    c.is_some_and(is_punct)
}

/// CommonMark's flanking rules for a delimiter run between `prev` and
/// `next`: whether it can open and whether it can close.
fn flanking(ch: char, prev: Option<char>, next: Option<char>) -> (bool, bool) {
    let left = !space(next) && (!punct(next) || space(prev) || punct(prev));
    let right = !space(prev) && (!punct(prev) || space(next) || punct(next));
    if ch == '_' {
        (
            left && (!right || punct(prev)),
            right && (!left || punct(next)),
        )
    } else {
        (left, right)
    }
}

/// The matching backtick run for one of `len` at byte `from`: the start
/// of the closing run.
fn closing_ticks(text: &str, from: usize, len: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            if i - start == len {
                return Some(start);
            }
        } else {
            i += 1;
        }
    }
    None
}

/// `text` split into items: code spans, escapes, URLs, delimiter runs and
/// text.
fn items(text: &str) -> Vec<Item> {
    let urls = bare_urls(text);
    let mut out: Vec<Item> = Vec::new();
    let mut lit = String::new();
    let push_lit = |lit: &mut String, out: &mut Vec<Item>| {
        if !lit.is_empty() {
            out.push(Item::Text(std::mem::take(lit)));
        }
    };
    let mut i = 0;
    while i < text.len() {
        if let Some(&(_, end)) = urls.iter().find(|&&(s, _)| s == i) {
            push_lit(&mut lit, &mut out);
            out.push(Item::Url(text[i..end].to_string()));
            i = end;
            continue;
        }
        let c = text[i..].chars().next().expect("in the text");
        match c {
            '\\' => {
                let next = text[i + 1..].chars().next();
                match next {
                    Some(n @ ('\\' | '*' | '_' | '`')) => {
                        lit.push(n);
                        i += 1 + n.len_utf8();
                    }
                    _ => {
                        lit.push('\\');
                        i += 1;
                    }
                }
            }
            '`' => {
                let start = i;
                let mut j = i;
                while text.as_bytes().get(j) == Some(&b'`') {
                    j += 1;
                }
                let len = j - start;
                match closing_ticks(text, j, len) {
                    Some(close) => {
                        push_lit(&mut lit, &mut out);
                        let mut code = &text[j..close];
                        // One space each side is stripped when both are
                        // there and the span is not all spaces.
                        if code.len() >= 2
                            && code.starts_with(' ')
                            && code.ends_with(' ')
                            && !code.trim().is_empty()
                        {
                            code = &code[1..code.len() - 1];
                        }
                        out.push(Item::Code(code.to_string()));
                        i = close + len;
                    }
                    None => {
                        lit.push_str(&text[start..j]);
                        i = j;
                    }
                }
            }
            '*' | '_' => {
                let start = i;
                let mut j = i;
                while text[j..].starts_with(c) {
                    j += 1;
                }
                let prev = text[..start].chars().next_back();
                let next = text[j..].chars().next();
                let (can_open, can_close) = flanking(c, prev, next);
                push_lit(&mut lit, &mut out);
                out.push(Item::Delim {
                    ch: c,
                    left: j - start,
                    can_open,
                    can_close,
                });
                i = j;
            }
            _ => {
                lit.push(c);
                i += c.len_utf8();
            }
        }
    }
    push_lit(&mut lit, &mut out);
    out
}

/// A matched pair: the opener's and closer's items and how many markers
/// (2 strong, 1 emphasis).
struct Match {
    open: usize,
    close: usize,
    n: usize,
}

/// Pair openers with closers, nearest first, as CommonMark's "process
/// emphasis" does (without its rule of three).
fn resolve(items: &mut [Item]) -> Vec<Match> {
    let mut matches = Vec::new();
    // Openers still able to match, by item index.
    let mut openers: Vec<usize> = Vec::new();
    for i in 0..items.len() {
        let (ch, can_open, can_close) = match items[i] {
            Item::Delim {
                ch,
                can_open,
                can_close,
                ..
            } => (ch, can_open, can_close),
            _ => continue,
        };
        if can_close {
            loop {
                let left = match items[i] {
                    Item::Delim { left, .. } => left,
                    _ => 0,
                };
                if left == 0 {
                    break;
                }
                let Some(pos) = openers
                    .iter()
                    .rposition(|&o| matches!(items[o], Item::Delim { ch: oc, .. } if oc == ch))
                else {
                    break;
                };
                let o = openers[pos];
                let o_left = match items[o] {
                    Item::Delim { left, .. } => left,
                    _ => 0,
                };
                let n = if o_left >= 2 && left >= 2 { 2 } else { 1 };
                if let Item::Delim { left, .. } = &mut items[o] {
                    *left -= n;
                }
                if let Item::Delim { left, .. } = &mut items[i] {
                    *left -= n;
                }
                matches.push(Match {
                    open: o,
                    close: i,
                    n,
                });
                // Openers between the pair can no longer match.
                openers.truncate(pos + 1);
                if o_left == n {
                    openers.pop();
                }
            }
        }
        let left = match items[i] {
            Item::Delim { left, .. } => left,
            _ => 0,
        };
        if can_open && left > 0 {
            openers.push(i);
        }
    }
    matches
}

/// `text`'s runs, its URLs as links into `links` when given, else plain.
fn inline_runs(text: &str, mut links: Option<&mut Vec<String>>) -> Vec<MdRun> {
    let mut items = items(text);
    let matches = resolve(&mut items);
    let mut runs: Vec<MdRun> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let (mut strong, mut em) = (0, 0);
        for m in &matches {
            if m.open < i && i < m.close {
                if m.n == 2 {
                    strong += 1;
                } else {
                    em += 1;
                }
            }
        }
        let style = match (strong > 0, em > 0) {
            (true, true) => MdStyle::BoldItalic,
            (true, false) => MdStyle::Bold,
            (false, true) => MdStyle::Italic,
            (false, false) => MdStyle::Regular,
        };
        let (text, style, link) = match item {
            Item::Text(t) => (t.clone(), style, None),
            Item::Code(t) => (t.clone(), MdStyle::Code, None),
            Item::Url(u) => {
                let link = links.as_deref_mut().map(|l| link_index(l, u));
                (u.clone(), style, link)
            }
            Item::Delim { ch, left, .. } => (ch.to_string().repeat(*left), style, None),
        };
        if text.is_empty() {
            continue;
        }
        match runs.last_mut() {
            Some(last) if last.style == style && last.link == link => last.text.push_str(&text),
            _ => runs.push(MdRun {
                link,
                ..MdRun::new(text, style)
            }),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, style: MdStyle) -> MdRun {
        MdRun::new(text, style)
    }

    #[test]
    fn a_paragraph_with_the_four_styles_and_one_link() {
        let doc = parse_minimal(
            "Some **bold**, *italic*, `code` and https://example.com.",
            0.5,
        );
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(
            doc.blocks[0].runs,
            [
                run("Some ", MdStyle::Regular),
                run("bold", MdStyle::Bold),
                run(", ", MdStyle::Regular),
                run("italic", MdStyle::Italic),
                run(", ", MdStyle::Regular),
                run("code", MdStyle::Code),
                run(" and ", MdStyle::Regular),
                MdRun {
                    link: Some(0),
                    ..run("https://example.com", MdStyle::Regular)
                },
                run(".", MdStyle::Regular),
            ]
        );
        assert_eq!(doc.links, ["https://example.com"]);
    }

    #[test]
    fn triple_markers_are_bold_italic_and_intraword_underscores_literal() {
        assert_eq!(parse_inline("***x***"), [run("x", MdStyle::BoldItalic)]);
        assert_eq!(
            parse_inline("snake_case_name"),
            [run("snake_case_name", MdStyle::Regular)]
        );
        assert_eq!(
            parse_inline("an **unmatched marker"),
            [run("an **unmatched marker", MdStyle::Regular)]
        );
        assert_eq!(
            parse_inline("**bold *both***"),
            [
                run("bold ", MdStyle::Bold),
                run("both", MdStyle::BoldItalic)
            ]
        );
        assert_eq!(
            parse_inline("__bold__ and _it_"),
            [
                run("bold", MdStyle::Bold),
                run(" and ", MdStyle::Regular),
                run("it", MdStyle::Italic)
            ]
        );
        assert_eq!(
            parse_inline("2 * 3 = 6"),
            [run("2 * 3 = 6", MdStyle::Regular)]
        );
    }

    #[test]
    fn code_spans_format_nothing_and_escapes_are_literal() {
        assert_eq!(
            parse_inline("`a **b**` \\*c\\* \\_d\\_ \\` \\\\ \\x"),
            [
                run("a **b**", MdStyle::Code),
                run(" *c* _d_ ` \\ \\x", MdStyle::Regular)
            ]
        );
        assert_eq!(
            parse_inline("an `unclosed span"),
            [run("an `unclosed span", MdStyle::Regular)]
        );
        assert_eq!(parse_inline("`` a`b ``"), [run("a`b", MdStyle::Code)]);
    }

    #[test]
    fn two_paragraphs_give_one_block_gap_and_soft_breaks_are_spaces() {
        let doc = parse_minimal("one\ntwo\n\n\nthree", 0.5);
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].runs, [run("one two", MdStyle::Regular)]);
        assert_eq!((doc.blocks[0].gap, doc.blocks[1].gap), (0.0, 0.5));
    }

    #[test]
    fn block_syntax_is_literal() {
        let doc = parse_minimal("# Title\n\n- item\n\n```\nfn x()\n```", 0.5);
        let texts: Vec<String> = doc
            .blocks
            .iter()
            .map(|b| b.runs.iter().map(|r| r.text.as_str()).collect())
            .collect();
        // A fence's lines are joined, and its backticks then pair as an
        // inline code span (CommonMark's inline rule), so its text is code.
        assert_eq!(texts, ["# Title", "- item", "fn x()"]);
        assert_eq!(doc.blocks[2].runs[0].style, MdStyle::Code);
        assert!(doc.blocks.iter().all(|b| b.marker.is_none() && !b.pre));
        assert_eq!(parse_minimal("", 0.5), MdDocument::default());
        let link = parse_minimal("[text](https://a.example)", 0.5);
        assert_eq!(link.links, ["https://a.example"]);
        assert_eq!(link.blocks[0].runs[0], run("[text](", MdStyle::Regular));
    }

    #[test]
    fn odd_and_multibyte_input_does_not_panic() {
        for text in [
            "é**ü**`ß`_ñ_ https://ü.example/x…",
            "\\",
            "*",
            "**",
            "`",
            "_*_*",
            "***a**b*",
            "a\u{2026}*\u{1F600}*",
        ] {
            let runs = parse_inline(text);
            assert!(runs.iter().all(|r| !r.text.is_empty()), "{text}");
        }
        assert_eq!(
            parse_inline("é**ü**"),
            [run("é", MdStyle::Regular), run("ü", MdStyle::Bold)]
        );
    }

    #[test]
    fn parse_inline_makes_the_url_plain() {
        assert_eq!(
            parse_inline("see https://example.com/a_b_c\nnow"),
            [run("see https://example.com/a_b_c now", MdStyle::Regular)]
        );
    }
}
