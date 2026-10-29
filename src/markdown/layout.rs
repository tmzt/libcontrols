//! Wrapping a document to a width (flankr-markdown's `wrap`, moved), with
//! no measure callback: a [`MarkdownMeasure`] measures with the faces it is
//! given, or the context's estimate without them (the plan's decision 7).

use crate::theme::MarkdownResolved;

use super::faces::MarkdownFaces;
use super::model::{MdBlock, MdDocument, MdLaid, MdLine, MdRun, MdStyle};

/// How Markdown text is measured: by its faces' shapers, or without faces
/// at `0.55` a character (`DrawContext::text_advance` without a shaper) and
/// code at the theme's `code_cell_em` a character, as flankr's app measures.
#[derive(Clone, Copy, Debug)]
pub struct MarkdownMeasure<'a> {
    faces: Option<&'a MarkdownFaces>,
    px: f32,
    code_cell_em: f32,
    quote_indent_chars: u32,
}

impl<'a> MarkdownMeasure<'a> {
    /// A measure at `px` by `faces` when given, with `theme`'s code cell and
    /// quote indent.
    pub fn new(faces: Option<&'a MarkdownFaces>, px: f32, theme: &MarkdownResolved) -> Self {
        Self {
            faces,
            px,
            code_cell_em: theme.code_cell_em,
            quote_indent_chars: theme.quote_indent_chars,
        }
    }

    /// The type size it measures at.
    pub fn px(&self) -> f32 {
        self.px
    }

    /// `text`'s width in `style`. With faces, code is the terminal face's
    /// cell a character, as it is drawn.
    pub fn width(&self, text: &str, style: MdStyle) -> f32 {
        let chars = text.chars().count() as f32;
        match (self.faces, style) {
            (Some(f), MdStyle::Code) => chars * self.px * f.code.shaper.cell_em(),
            (Some(f), style) => f.prose(style).shaper.shape(text).advance_px(self.px),
            (None, MdStyle::Code) => chars * self.px * self.code_cell_em,
            (None, _) => chars * self.px * 0.55,
        }
    }

    /// A block's indent: its enclosing list items' markers and its quotes.
    fn block_indent(&self, block: &MdBlock) -> f32 {
        let mut list = 0.0;
        for level in 0..block.list_depth as usize {
            let marker = block
                .outer_markers
                .get(level)
                .map_or("\u{2022} ", String::as_str);
            list += self.width(marker, MdStyle::Regular);
        }
        let quote = " ".repeat(self.quote_indent_chars as usize);
        list + block.quote as f32 * self.width(&quote, MdStyle::Regular)
    }
}

/// `doc` laid out in lines at most `width` wide, measured by `m`. Words
/// wrap at spaces; a word wider than the line breaks by character. A list
/// item's lines are indented under its marker.
pub fn lay_out(doc: &MdDocument, width: f32, m: &MarkdownMeasure) -> MdLaid {
    let mut lines = Vec::new();
    for block in &doc.blocks {
        wrap(block, width, m, &mut lines);
    }
    MdLaid {
        lines,
        links: doc.links.clone(),
    }
}

/// A word, a space between words, or a forced break.
enum Token {
    Word(Vec<MdRun>),
    Space(MdRun),
    Break,
}

/// `block`'s runs as words and spaces; in a code block, spaces are part
/// of the words. A `'\n'` is a forced break.
fn tokens(block: &MdBlock) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word: Vec<MdRun> = Vec::new();
    let end_word = |word: &mut Vec<MdRun>, out: &mut Vec<Token>| {
        if !word.is_empty() {
            out.push(Token::Word(std::mem::take(word)));
        }
    };
    for run in &block.runs {
        for c in run.text.chars() {
            if c == '\n' {
                end_word(&mut word, &mut out);
                out.push(Token::Break);
                continue;
            }
            if c.is_whitespace() && !block.pre {
                end_word(&mut word, &mut out);
                if !matches!(out.last(), Some(Token::Space(_))) {
                    out.push(Token::Space(MdRun {
                        text: " ".to_string(),
                        ..run.clone()
                    }));
                }
                continue;
            }
            match word.last_mut() {
                Some(last) if same(last, run) => last.text.push(c),
                _ => word.push(MdRun {
                    text: c.to_string(),
                    ..run.clone()
                }),
            }
        }
    }
    end_word(&mut word, &mut out);
    out
}

/// Two runs that draw alike.
fn same(a: &MdRun, b: &MdRun) -> bool {
    a.style == b.style && a.link == b.link && a.dim == b.dim
}

/// `run` added to `line`, merged into the last run when they draw alike.
fn append(line: &mut Vec<MdRun>, run: MdRun) {
    match line.last_mut() {
        Some(last) if same(last, &run) => last.text.push_str(&run.text),
        _ => line.push(run),
    }
}

/// `block`'s lines, wrapped to `width`, added to `out`.
fn wrap(block: &MdBlock, width: f32, m: &MarkdownMeasure, out: &mut Vec<MdLine>) {
    let measure = |text: &str, style: MdStyle| m.width(text, style);
    let indent = m.block_indent(block);
    let dim = block.quote > 0;
    let hang = block
        .marker
        .as_deref()
        .map_or(0.0, |mk| measure(mk, MdStyle::Regular));
    let avail = (width - indent - hang).max(1.0);
    let mut lines: Vec<Vec<MdRun>> = Vec::new();
    let mut cur: Vec<MdRun> = Vec::new();
    let mut cur_w = 0.0;
    let mut space: Option<MdRun> = None;
    let width_of = |runs: &[MdRun]| -> f32 { runs.iter().map(|r| measure(&r.text, r.style)).sum() };
    for token in tokens(block) {
        match token {
            Token::Space(run) => {
                if !cur.is_empty() {
                    space = Some(run);
                }
            }
            Token::Break => {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
                space = None;
            }
            Token::Word(word) => {
                let w = width_of(&word);
                let sw = space.as_ref().map_or(0.0, |s| measure(&s.text, s.style));
                if !cur.is_empty() && cur_w + sw + w > avail {
                    lines.push(std::mem::take(&mut cur));
                    cur_w = 0.0;
                    space = None;
                }
                if let Some(s) = space.take()
                    && !cur.is_empty()
                {
                    append(&mut cur, s);
                    cur_w += sw;
                }
                if cur.is_empty() && w > avail {
                    // Too wide for any line: broken by character.
                    for run in word {
                        for c in run.text.chars() {
                            let piece = MdRun {
                                text: c.to_string(),
                                ..run.clone()
                            };
                            let cw = measure(&piece.text, piece.style);
                            if !cur.is_empty() && cur_w + cw > avail {
                                lines.push(std::mem::take(&mut cur));
                                cur_w = 0.0;
                            }
                            append(&mut cur, piece);
                            cur_w += cw;
                        }
                    }
                } else {
                    for run in word {
                        append(&mut cur, run);
                    }
                    cur_w += w;
                }
            }
        }
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    for (i, runs) in lines.into_iter().enumerate() {
        let mut line = MdLine {
            runs: Vec::new(),
            indent: indent + hang,
            gap_before: 0.0,
        };
        if i == 0 {
            line.gap_before = block.gap;
            if let Some(marker) = &block.marker {
                line.indent = indent;
                line.runs.push(MdRun {
                    dim,
                    ..MdRun::new(marker.clone(), MdStyle::Regular)
                });
            }
        }
        // The marker stays a run of its own.
        let marker_runs = line.runs.len();
        for run in runs {
            if line.runs.len() == marker_runs {
                line.runs.push(run);
            } else {
                append(&mut line.runs, run);
            }
        }
        out.push(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::inline::parse_minimal;

    /// At `PX`, 11 a character in every style: prose is 0.55 a character
    /// and the code cell is set to match.
    const PX: f32 = 20.0;
    const CH: f32 = 11.0;

    fn unit() -> MarkdownResolved {
        MarkdownResolved {
            code_cell_em: 0.55,
            ..MarkdownResolved::default()
        }
    }

    fn lay(md: &str, width: f32) -> MdLaid {
        let r = unit();
        let m = MarkdownMeasure::new(None, PX, &r);
        lay_out(&parse_minimal(md, 0.5), width * CH, &m)
    }

    fn texts(laid: &MdLaid) -> Vec<String> {
        laid.lines
            .iter()
            .map(|l| l.runs.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn words_wrap_at_spaces_and_a_long_word_breaks_by_character() {
        let laid = lay("one two three four", 9.0);
        assert_eq!(texts(&laid), ["one two", "three", "four"]);
        let laid = lay("a abcdefghijkl b", 5.0);
        assert_eq!(texts(&laid), ["a", "abcde", "fghij", "kl b"]);
        // Styles stay with their words across the wrap.
        let laid = lay("plain **bold words**", 8.0);
        assert_eq!(laid.lines[1].runs, [MdRun::new("bold", MdStyle::Bold)]);
        assert_eq!(laid.lines[2].runs, [MdRun::new("words", MdStyle::Bold)]);
    }

    #[test]
    fn a_pre_block_keeps_its_lines_and_breaks_long_ones() {
        let block = MdBlock {
            runs: vec![MdRun::new("ab  c\nabcdefgh", MdStyle::Code)],
            pre: true,
            ..MdBlock::default()
        };
        let doc = MdDocument {
            blocks: vec![block],
            links: Vec::new(),
        };
        let r = unit();
        let laid = lay_out(&doc, 5.0 * CH, &MarkdownMeasure::new(None, PX, &r));
        assert_eq!(texts(&laid), ["ab  c", "abcde", "fgh"]);
    }

    #[test]
    fn two_paragraphs_give_one_block_gap() {
        let laid = lay("one\n\ntwo", 80.0);
        let gaps: Vec<f32> = laid.lines.iter().map(|l| l.gap_before).collect();
        assert_eq!(gaps, [0.0, 0.5]);
        assert_eq!(lay("", 10.0), MdLaid::default());
    }

    #[test]
    fn without_faces_prose_is_0_55_and_code_the_cell() {
        let r = MarkdownResolved::default();
        let m = MarkdownMeasure::new(None, 20.0, &r);
        assert_eq!(m.width("abcd", MdStyle::Bold), 4.0 * 20.0 * 0.55);
        assert_eq!(m.width("abcd", MdStyle::Code), 4.0 * 20.0 * 0.6);
    }
}
