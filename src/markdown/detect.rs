//! Markdown detection (`cmark` only), moved from flankr-markdown: how much
//! of a text is Markdown formatting, by pulldown-cmark's parse (flankr plan
//! 20261026_01, decisions 2 and 4).

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

/// The default percentage of Markdown formatting usage required to switch to Markdown rendering.
pub const DEFAULT_MARKDOWN_THRESHOLD_PCT: f32 = 15.0;

/// Metrics measuring Markdown feature density and character coverage in a text snippet
/// (plan 20261026_01, decision 2).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct MarkdownScore {
    /// Non-whitespace characters belonging to recognized Markdown syntax
    /// or formatted spans.
    pub formatted_chars: usize,
    /// Total non-whitespace characters in the text.
    pub total_chars: usize,
    /// Number of lines with structural Markdown indicators (headings, lists,
    /// quotes, fences).
    pub structural_lines: usize,
    /// Total non-empty lines in the text.
    pub total_lines: usize,
    /// Whether the text contains at least one fenced code block.
    pub has_fenced_code: bool,
}

impl MarkdownScore {
    /// Ratio of formatted characters to total characters (0.0 to 1.0).
    pub fn char_ratio(&self) -> f32 {
        if self.total_chars == 0 {
            0.0
        } else {
            (self.formatted_chars as f32 / self.total_chars as f32).min(1.0)
        }
    }

    /// Ratio of structural lines to total lines (0.0 to 1.0).
    pub fn line_ratio(&self) -> f32 {
        if self.total_lines == 0 {
            0.0
        } else {
            (self.structural_lines as f32 / self.total_lines as f32).min(1.0)
        }
    }

    /// Whether Markdown usage exceeds `threshold_pct` (e.g. 15.0%).
    pub fn is_markdown(&self, threshold_pct: f32) -> bool {
        if self.has_fenced_code {
            return true;
        }
        if self.total_chars >= 20 && self.char_ratio() * 100.0 >= threshold_pct {
            return true;
        }
        if self.total_lines >= 3 && self.line_ratio() >= 0.25 {
            return true;
        }
        false
    }
}

/// Computes Markdown usage metrics for `text` (plan 20261026_01, decision 2).
pub fn score_markdown(text: &str) -> MarkdownScore {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return MarkdownScore::default();
    }

    let total_chars = text.chars().filter(|c| !c.is_whitespace()).count();
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();

    let mut line_starts = Vec::with_capacity(lines.len());
    let mut byte_idx = 0;
    for l in &lines {
        line_starts.push(byte_idx);
        byte_idx += l.len();
        if let Some(rest) = text.get(byte_idx..) {
            if rest.starts_with("\r\n") {
                byte_idx += 2;
            } else if rest.starts_with('\n') || rest.starts_with('\r') {
                byte_idx += 1;
            }
        }
    }

    let find_line = |byte_pos: usize| -> usize {
        match line_starts.binary_search(&byte_pos) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        }
    };

    let mut formatted_bytes = vec![false; text.len()];
    let mut structural_line_flags = vec![false; lines.len()];
    let mut has_fenced_code = false;

    let mut strong_depth = 0usize;
    let mut emphasis_depth = 0usize;
    let mut heading_depth = 0usize;
    let mut quote_depth = 0usize;
    let mut link_depth = 0usize;
    let mut fenced_code_depth = 0usize;

    for (event, range) in Parser::new(text).into_offset_iter() {
        let r_start = range.start.min(text.len());
        let r_end = range.end.min(text.len());

        let mark_range = |start: usize, end: usize, flags: &mut [bool]| {
            if start < end && end <= flags.len() {
                for b in &mut flags[start..end] {
                    *b = true;
                }
            }
        };

        let mark_lines = |start: usize, end: usize, flags: &mut [bool]| {
            let l_start = find_line(start);
            let l_end = find_line(end.saturating_sub(1).max(start));
            for l in l_start..=l_end.min(flags.len().saturating_sub(1)) {
                flags[l] = true;
            }
        };

        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                has_fenced_code = true;
                fenced_code_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::End(TagEnd::CodeBlock) if fenced_code_depth > 0 => {
                fenced_code_depth = fenced_code_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::Start(Tag::Heading { .. }) => {
                heading_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::End(TagEnd::Heading(_)) => {
                heading_depth = heading_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                quote_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                quote_depth = quote_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::Start(Tag::Item) => {
                mark_range(r_start, r_end, &mut formatted_bytes);
                mark_lines(r_start, r_end, &mut structural_line_flags);
            }
            Event::End(TagEnd::Item) => {
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::Start(Tag::Strong) => {
                strong_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::End(TagEnd::Strong) => {
                strong_depth = strong_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::Start(Tag::Emphasis) => {
                emphasis_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::End(TagEnd::Emphasis) => {
                emphasis_depth = emphasis_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::Start(Tag::Link { .. }) => {
                link_depth += 1;
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::End(TagEnd::Link) => {
                link_depth = link_depth.saturating_sub(1);
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::Code(_) => {
                mark_range(r_start, r_end, &mut formatted_bytes);
            }
            Event::Text(_) => {
                if fenced_code_depth > 0
                    || heading_depth > 0
                    || quote_depth > 0
                    || strong_depth > 0
                    || emphasis_depth > 0
                    || link_depth > 0
                {
                    mark_range(r_start, r_end, &mut formatted_bytes);
                }
                if fenced_code_depth > 0 || heading_depth > 0 || quote_depth > 0 {
                    mark_lines(r_start, r_end, &mut structural_line_flags);
                }
            }
            _ => {}
        }
    }

    let formatted_chars = text
        .char_indices()
        .filter(|&(idx, c)| {
            !c.is_whitespace() && formatted_bytes.get(idx).copied().unwrap_or(false)
        })
        .count();

    let structural_lines = structural_line_flags
        .iter()
        .enumerate()
        .filter(|&(idx, &is_struct)| is_struct && !lines[idx].trim().is_empty())
        .count();

    MarkdownScore {
        formatted_chars,
        total_chars,
        structural_lines,
        total_lines,
        has_fenced_code,
    }
}

/// Returns whether `text` exceeds `threshold_pct` Markdown usage (plan 20261026_01, decision 4).
pub fn is_markdown(text: &str, threshold_pct: f32) -> bool {
    score_markdown(text).is_markdown(threshold_pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_detection_fenced_code_blocks() {
        let code = "Here is the code:\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\nDone.";
        let score = score_markdown(code);
        assert!(score.has_fenced_code);
        assert!(score.is_markdown(DEFAULT_MARKDOWN_THRESHOLD_PCT));
        assert!(is_markdown(code, DEFAULT_MARKDOWN_THRESHOLD_PCT));
    }

    #[test]
    fn markdown_detection_bullet_and_numbered_lists() {
        let list = "Steps to follow:\n- First run tests\n- Inspect output\n- Commit changes";
        let score = score_markdown(list);
        assert_eq!(score.total_lines, 4);
        assert_eq!(score.structural_lines, 3);
        assert!(score.line_ratio() >= 0.7);
        assert!(is_markdown(list, DEFAULT_MARKDOWN_THRESHOLD_PCT));

        let numbered = "Plan:\n1. Update manifest\n2. Run linter\n3. Push main";
        assert!(is_markdown(numbered, DEFAULT_MARKDOWN_THRESHOLD_PCT));
    }

    #[test]
    fn markdown_detection_headings_and_emphasis() {
        let doc = "# Architecture Plan\nThis document describes **core state** and *pipeline* structures.";
        let score = score_markdown(doc);
        assert!(score.char_ratio() > 0.3);
        assert!(is_markdown(doc, DEFAULT_MARKDOWN_THRESHOLD_PCT));
    }

    #[test]
    fn markdown_detection_plain_terminal_commands_not_markdown() {
        // Glob pattern with asterisk and flags should not trigger markdown
        let terminal = "$ ls -la *.rs\n-rw-r--r--  1 user staff 1024 Oct 24 main.rs\n-rw-r--r--  1 user staff 2048 Oct 24 lib.rs";
        let score = score_markdown(terminal);
        assert!(!score.has_fenced_code);
        assert!(!is_markdown(terminal, DEFAULT_MARKDOWN_THRESHOLD_PCT));

        let diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n-let a = 1;\n+let b = 2;";
        assert!(!is_markdown(diff, DEFAULT_MARKDOWN_THRESHOLD_PCT));

        let math = "The formula is simple: 2 * 3 = 6, 4 * 5 = 20.";
        assert!(!is_markdown(math, DEFAULT_MARKDOWN_THRESHOLD_PCT));
    }

    #[test]
    fn markdown_detection_empty_and_short_text() {
        assert!(!is_markdown("", DEFAULT_MARKDOWN_THRESHOLD_PCT));
        assert!(!is_markdown("   \n\t  ", DEFAULT_MARKDOWN_THRESHOLD_PCT));
        assert!(!is_markdown("ok", DEFAULT_MARKDOWN_THRESHOLD_PCT));
    }
}
