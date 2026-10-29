//! The `cmark` parser (the plan's decision 5): flankr-markdown's `Builder`,
//! moved, emitting an [`MdDocument`]. pulldown-cmark is created with no
//! extensions (CommonMark, no GFM): tables, strikethrough and task lists stay
//! literal text. Headings are bold, block quotes dim and indented a level
//! each, code blocks in [`MdStyle::Code`] keeping their lines, list items
//! "• " or "N. " with a hanging indent; links and bare URLs are collected in
//! order. Raw HTML is its literal text, and an image its alt text linked to
//! its URL.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use super::model::{MdBlock, MdDocument, MdRun, MdStyle, bare_urls, link_index};

/// `md` parsed with pulldown-cmark, with the given gaps (in lines).
pub(crate) fn parse_cmark(md: &str, block_gap: f32, item_gap: f32) -> MdDocument {
    let mut b = Builder::new(block_gap, item_gap);
    for event in Parser::new(md) {
        b.event(event);
    }
    b.flush();
    MdDocument {
        blocks: b.blocks,
        links: b.links,
    }
}

struct Builder {
    block_gap: f32,
    item_gap: f32,
    blocks: Vec<MdBlock>,
    current: Option<MdBlock>,
    links: Vec<String>,
    strong: u32,
    emphasis: u32,
    quote: u32,
    link: Option<usize>,
    code_block: bool,
    /// The open lists' next numbers (`None` for bullets).
    lists: Vec<Option<u64>>,
    /// The open items' markers, outermost first.
    items: Vec<String>,
    /// The next item is its list's first.
    first_item: bool,
    /// The last HTML block line ended with a newline: the next starts a line.
    html_break: bool,
}

impl Builder {
    fn new(block_gap: f32, item_gap: f32) -> Self {
        Builder {
            block_gap,
            item_gap,
            blocks: Vec::new(),
            current: None,
            links: Vec::new(),
            strong: 0,
            emphasis: 0,
            quote: 0,
            link: None,
            code_block: false,
            lists: Vec::new(),
            items: Vec::new(),
            first_item: false,
            html_break: false,
        }
    }

    fn style(&self) -> MdStyle {
        if self.code_block {
            return MdStyle::Code;
        }
        match (self.strong > 0, self.emphasis > 0) {
            (true, true) => MdStyle::BoldItalic,
            (true, false) => MdStyle::Bold,
            (false, true) => MdStyle::Italic,
            (false, false) => MdStyle::Regular,
        }
    }

    /// A new block where text starts now: inside the open items and quotes.
    fn new_block(&self, marker: Option<String>, gap: f32, pre: bool) -> MdBlock {
        MdBlock {
            runs: Vec::new(),
            list_depth: self.items.len().min(u8::MAX as usize) as u8,
            outer_markers: self.items.clone(),
            quote: self.quote.min(u8::MAX as u32) as u8,
            marker,
            pre,
            gap,
        }
    }

    /// The gap a block gets: none for the first.
    fn gap(&self, gap: f32) -> f32 {
        if self.blocks.is_empty() && self.current.is_none() {
            0.0
        } else {
            gap
        }
    }

    /// A block to add text to: the open one while it is still empty (an
    /// item's first paragraph), otherwise a new one.
    fn open(&mut self, pre: bool) {
        if self.current.as_ref().is_some_and(|b| b.runs.is_empty()) {
            if let Some(b) = &mut self.current {
                b.pre = pre;
            }
            return;
        }
        self.flush();
        self.current = Some(self.new_block(None, self.gap(self.block_gap), pre));
    }

    /// Ends the open block.
    fn flush(&mut self) {
        if let Some(block) = self.current.take()
            && !block.runs.is_empty()
        {
            self.blocks.push(block);
        }
    }

    /// The block text goes to, opened if none is.
    fn block(&mut self) -> &mut MdBlock {
        if self.current.is_none() {
            self.open(self.code_block);
        }
        self.current.as_mut().expect("opened")
    }

    fn push_run(&mut self, text: &str, style: MdStyle, link: Option<usize>) {
        if text.is_empty() {
            return;
        }
        let dim = self.quote > 0;
        self.block().runs.push(MdRun {
            text: text.to_string(),
            style,
            link,
            dim,
        });
    }

    /// A hard line break: a `'\n'` run.
    fn push_break(&mut self) {
        let style = self.style();
        let dim = self.quote > 0;
        self.block().runs.push(MdRun {
            text: "\n".to_string(),
            style,
            link: None,
            dim,
        });
    }

    /// Lines of literal text, broken where it has newlines.
    fn push_lines(&mut self, text: &str, style: MdStyle) {
        let text = text.strip_suffix('\n').unwrap_or(text);
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.push_break();
            }
            self.push_run(line, style, self.link);
        }
    }

    /// Text, with its bare URLs as links unless it is already a link's.
    fn push_text(&mut self, text: &str) {
        let style = self.style();
        if self.link.is_some() {
            return self.push_run(text, style, self.link);
        }
        let mut from = 0;
        for (start, end) in bare_urls(text) {
            self.push_run(&text[from..start], style, None);
            let link = link_index(&mut self.links, &text[start..end]);
            self.push_run(&text[start..end], style, Some(link));
            from = end;
        }
        self.push_run(&text[from..], style, None);
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.code_block => self.push_lines(&text, MdStyle::Code),
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => self.push_run(&text, MdStyle::Code, self.link),
            // Shown as written, never interpreted.
            Event::Html(text) => {
                if self.html_break {
                    self.push_break();
                }
                self.push_lines(&text, self.style());
                self.html_break = text.ends_with('\n');
            }
            Event::InlineHtml(text) => self.push_lines(&text, self.style()),
            Event::SoftBreak => {
                let style = self.style();
                self.push_run(" ", style, self.link);
            }
            Event::HardBreak => self.push_break(),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => self.open(false),
            Tag::Heading { .. } => {
                self.open(false);
                self.strong += 1;
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(_) => {
                self.open(true);
                self.code_block = true;
            }
            Tag::List(first) => {
                self.flush();
                self.lists.push(first);
                self.first_item = true;
            }
            Tag::Item => {
                self.flush();
                let marker = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "\u{2022} ".to_string(),
                };
                let gap = self.gap(if self.first_item {
                    self.block_gap
                } else {
                    self.item_gap
                });
                self.first_item = false;
                self.current = Some(self.new_block(Some(marker.clone()), gap, false));
                self.items.push(marker);
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.link = Some(link_index(&mut self.links, &dest_url));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush(),
            TagEnd::HtmlBlock => {
                self.html_break = false;
                self.flush();
            }
            TagEnd::Heading(_) => {
                self.strong = self.strong.saturating_sub(1);
                self.flush();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.code_block = false;
                self.flush();
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                self.first_item = false;
            }
            TagEnd::Item => {
                self.flush();
                self.items.pop();
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Link | TagEnd::Image => self.link = None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    //! flankr-markdown's tests, moved: one unit a character becomes 11 px at
    //! 20 px (0.55 a character, the code cell set to match).
    use super::*;
    use crate::markdown::layout::{MarkdownMeasure, lay_out};
    use crate::markdown::model::MdLaid;
    use crate::theme::MarkdownResolved;

    const BLOCK_GAP: f32 = 0.5;
    const ITEM_GAP: f32 = 0.15;
    const CH: f32 = 11.0;

    fn lay(md: &str, width: f32) -> MdLaid {
        let r = MarkdownResolved {
            code_cell_em: 0.55,
            ..MarkdownResolved::default()
        };
        let m = MarkdownMeasure::new(None, 20.0, &r);
        lay_out(&parse_cmark(md, BLOCK_GAP, ITEM_GAP), width * CH, &m)
    }

    fn texts(laid: &MdLaid) -> Vec<String> {
        laid.lines
            .iter()
            .map(|l| l.runs.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    fn run(text: &str, style: MdStyle) -> MdRun {
        MdRun::new(text, style)
    }

    #[test]
    fn a_paragraph_with_bold_italic_code_and_a_link() {
        let laid = lay(
            "Some **bold**, *italic*, `code` and [a link](https://example.com).",
            200.0,
        );
        assert_eq!(laid.lines.len(), 1);
        let line = &laid.lines[0];
        assert_eq!(line.gap_before, 0.0);
        assert_eq!(
            line.runs,
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
                    ..run("a link", MdStyle::Regular)
                },
                run(".", MdStyle::Regular),
            ]
        );
        assert_eq!(laid.links, ["https://example.com"]);
    }

    #[test]
    fn bold_with_italic_inside_is_bold_italic() {
        let laid = lay("**bold *both***", 80.0);
        assert_eq!(
            laid.lines[0].runs,
            [
                run("bold ", MdStyle::Bold),
                run("both", MdStyle::BoldItalic)
            ]
        );
    }

    #[test]
    fn headings_are_bold_quotes_dim_and_code_blocks_keep_their_lines() {
        let laid = lay(
            "# Title\n\nText.\n\n> quoted\n\n```\nfn main() {\n    go();\n}\n```\n",
            80.0,
        );
        assert_eq!(
            texts(&laid),
            ["Title", "Text.", "quoted", "fn main() {", "    go();", "}"]
        );
        assert_eq!(laid.lines[0].runs, [run("Title", MdStyle::Bold)]);
        assert_eq!(laid.lines[1].gap_before, BLOCK_GAP);
        let quote = &laid.lines[2];
        assert!(quote.runs.iter().all(|r| r.dim));
        assert_eq!(quote.indent, 4.0 * CH);
        for line in &laid.lines[3..] {
            assert!(line.runs.iter().all(|r| r.style == MdStyle::Code));
            assert_eq!(line.indent, 0.0);
        }
        assert_eq!(laid.lines[3].gap_before, BLOCK_GAP);
        assert_eq!(laid.lines[4].gap_before, 0.0);
    }

    #[test]
    fn html_is_shown_as_its_literal_text() {
        let laid = lay("Hi <b>there</b> & <script>x</script>.", 80.0);
        assert_eq!(texts(&laid), ["Hi <b>there</b> & <script>x</script>."]);
        let laid = lay("<div class=\"a\">\nblock\n</div>\n\nAfter.", 80.0);
        assert_eq!(
            texts(&laid),
            ["<div class=\"a\">", "block", "</div>", "After."]
        );
    }

    #[test]
    fn an_image_is_its_alt_text_linked_to_its_url() {
        let laid = lay("![a chart](https://example.com/c.png)", 80.0);
        assert_eq!(texts(&laid), ["a chart"]);
        assert_eq!(laid.lines[0].runs[0].link, Some(0));
        assert_eq!(laid.links, ["https://example.com/c.png"]);
    }

    #[test]
    fn words_wrap_at_spaces_and_a_long_word_breaks_by_character() {
        let laid = lay("one two three four", 9.0);
        assert_eq!(texts(&laid), ["one two", "three", "four"]);
        let laid = lay("a abcdefghijkl b", 5.0);
        assert_eq!(texts(&laid), ["a", "abcde", "fghij", "kl b"]);
        // A long code line breaks too.
        let laid = lay("```\nabcdefgh\n```", 5.0);
        assert_eq!(texts(&laid), ["abcde", "fgh"]);
        // Styles stay with their words across the wrap.
        let laid = lay("plain **bold words**", 8.0);
        assert_eq!(laid.lines[1].runs, [run("bold", MdStyle::Bold)]);
        assert_eq!(laid.lines[2].runs, [run("words", MdStyle::Bold)]);
    }

    #[test]
    fn list_items_are_indented_under_their_markers() {
        let laid = lay("- first item\n- second\n  - nested\n\n1. one\n2. two", 10.0);
        assert_eq!(
            texts(&laid),
            [
                "\u{2022} first",
                "item",
                "\u{2022} second",
                "\u{2022} nested",
                "1. one",
                "2. two"
            ]
        );
        let indents: Vec<f32> = laid.lines.iter().map(|l| l.indent).collect();
        assert_eq!(indents, [0.0, 2.0 * CH, 0.0, 2.0 * CH, 0.0, 0.0]);
        let gaps: Vec<f32> = laid.lines.iter().map(|l| l.gap_before).collect();
        assert_eq!(gaps, [0.0, 0.0, ITEM_GAP, BLOCK_GAP, BLOCK_GAP, ITEM_GAP]);
        assert_eq!(laid.lines[0].runs[0], run("\u{2022} ", MdStyle::Regular));
    }

    #[test]
    fn a_numbered_items_nested_list_hangs_under_its_number() {
        let laid = lay("1. one\n   - sub", 20.0);
        assert_eq!(texts(&laid), ["1. one", "\u{2022} sub"]);
        assert_eq!(laid.lines[1].indent, 3.0 * CH);
    }

    #[test]
    fn links_are_collected_in_order_with_bare_urls() {
        let laid = lay(
            "See https://b.example/x. Then [docs](https://a.example), \
             <https://c.example> and https://b.example/x again; not xhttps://d.example.",
            400.0,
        );
        assert_eq!(
            laid.links,
            [
                "https://b.example/x",
                "https://a.example",
                "https://c.example"
            ]
        );
        let linked: Vec<(String, usize)> = laid.lines[0]
            .runs
            .iter()
            .filter_map(|r| r.link.map(|l| (r.text.clone(), l)))
            .collect();
        assert_eq!(
            linked,
            [
                ("https://b.example/x".to_string(), 0),
                ("docs".to_string(), 1),
                ("https://c.example".to_string(), 2),
                ("https://b.example/x".to_string(), 0),
            ]
        );
    }

    #[test]
    fn empty_text_lays_out_nothing() {
        assert_eq!(lay("", 10.0), MdLaid::default());
    }

    #[test]
    fn a_gfm_table_stays_literal() {
        let laid = lay("| a | b |\n|---|---|\n| 1 | 2 |", 400.0);
        assert_eq!(texts(&laid), ["| a | b | |---|---| | 1 | 2 |"]);
        assert!(
            laid.lines[0]
                .runs
                .iter()
                .all(|r| r.style == MdStyle::Regular)
        );
        let laid = lay("~~struck~~ and - [ ] task", 400.0);
        assert_eq!(texts(&laid), ["~~struck~~ and - [ ] task"]);
    }
}
