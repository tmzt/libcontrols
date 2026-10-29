---
title: MarkdownView, basic Markdown behind a feature
date: 2026-10-28
status: Draft
covers: >
  A `markdown` module in libcontrols: flankr's Markdown layout
  (flankr-markdown's `lay_out`) and its `MarkdownView` control moved here,
  basic formatting only (paragraphs, headings, emphasis, strong, inline
  code, code blocks, lists, block quotes, links; no HTML, no images, no
  tables). It works on libcontrols' own small block and inline model and
  measures with its own faces, so the app's measure callback is gone. It is
  themed per spec 8.5 (`ControlTheme::markdown`) instead of the app's
  `theme()`. Links are reported by index, as now. Two cargo features: `markdown`
  enables the module with a minimal built-in parser (paragraphs with bold,
  italic, code and bare links) and no new dependency; `cmark` (depending on
  `markdown`) adds pulldown-cmark 0.13 (default features off) for full
  CommonMark blocks. There is an inline-only entry point for the Scrollbar's
  `BookmarkText::Markdown` pills. The theme types are unconditional, the
  Scrollbar is unconditional, and there are no default features. Additive:
  the release after the Scrollbar's (0.10.6 as numbered here). Steps and
  tests.
sources:
  - >
    The owner (2026-10-28), verbatim: "we could move the markdown render as
    well, if it doesn't have heavy callbacks"; then "but yes, it should be
    basic formatting only"; then, on the dependency, "feature based with
    cmark as a further feature". These ask for this plan as a Draft. They
    are not an approval to build it, and the status stays `Draft` until the
    owner approves it.
  - >
    The owner (2026-10-28), verbatim, on the Scrollbar plan: "we should
    backport the advanced scrollbar to libcontrols and use it for the file
    viewer, with the optional features like labels"; "add_bookmark(index_id,
    text/markdown (enum))" (the `BookmarkText::Markdown` this draws).
  - >
    flankr-markdown, ~/src/flankr/crates/flankr-markdown/src/lib.rs (1048
    lines, read-only): pulldown-cmark 0.13 with default features off
    (CommonMark, no GFM); `Style { Regular, Bold, Italic, BoldItalic, Code
    }`, `Run { text, style, link, dim }`, `Line { runs, indent, gap_before
    }`, `Laid { lines, links }`, `BLOCK_GAP` 0.5, `ITEM_GAP` 0.15; bare
    `http://`/`https://` URLs as links with trailing punctuation trimmed;
    `QUOTE_INDENT` four spaces; headings bold, quotes dim, code blocks in
    `Code` keeping their lines, list items "• " or "N. " with a hanging
    indent, HTML as its literal text, an image as its alt text linked to its
    URL; `lay_out(md, width, measure: &dyn Fn(&str, Style) -> f32)`, its
    only callback, a text-width measure; `score_markdown`, `is_markdown`,
    `MarkdownScore`, `DEFAULT_MARKDOWN_THRESHOLD_PCT` 15.0 (these also use
    pulldown-cmark's parser).
  - >
    flankr's app, ~/src/flankr/apps/companion/src/markdown_view.rs (389
    lines, read-only): `Face`, `CodeFace`, `Faces { regular, bold, italic,
    code, px_range }`, `Faces::metrics`, `measure(faces, text, style, px)`
    (code at `CELL_EM` 0.6 a character, else the shaper's advance, without
    faces `0.55` a character); `MarkdownView::new(md, x, y, width, px,
    color, faces)`, `height`, `rect`, `px`, `source`, `text`, `links`,
    `move_to`, `center`, `link_at`, `link_rect`; links in the app's
    `theme().link` and underlined (`px * 0.07`, at least 1, at `0.92` of
    the line), quotes in `theme().dim`; a link's tap area 15% above and 1.3
    lines tall; `Click` on a link answers `ControlAction::Activated { part:
    ControlPart::Custom(index) }`; line height `px * 1.35`. Used by the
    agent view (`lay_out`, `is_markdown`), the settings screen
    (`MarkdownView`, `measure`), the prompt history and ui.rs.
  - >
    flankr's markdown renderer plan,
    ~/src/flankr/docs/design/20261024_04_MARKDOWN_RENDERER_PLAN.md (In
    implementation), read-only: the plan that made the above.
  - >
    libcontrols at 022668c (0.10.3): Cargo.toml (no features today), the
    spec docs/design/20261028_02_LIBCONTROLS_SPEC.md (8.5), and the Drawer
    plan's 11:24 EDT amendment (the owner dropped a `gpu` feature: "drop the
    feature, libcontroks is gpu-based"; core parts are not behind
    features), src/traits.rs (`DrawContext` carries one `TextShaper`;
    `text_advance` falls back to `0.55` a character).
related:
  - docs/design/20261028_08_SCROLLBAR_PLAN.md (Draft; `BookmarkText::Markdown`
    is drawn through this plan's inline entry point, its decision 6)
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; amended
    when this is implemented)
  - ~/src/flankr/docs/design/20261024_04_MARKDOWN_RENDERER_PLAN.md (the
    renderer moved here; retired by a flankr plan once flankr's copy is
    removed)
  - ~/src/flankr/docs/design/20261028_05_BINARY_FILES_PLAN.md (the file
    preview, which would enable `cmark`)
---

# MarkdownView, basic Markdown behind a feature

## 1. What it is

flankr draws Markdown in several places: the agent view, the settings
screen, the prompt history, and soon the file preview and the scrollbar's
bookmarks. The layout lives in flankr-markdown and the control in the app.
The owner asked to move both into libcontrols, "if it doesn't have heavy
callbacks", with "basic formatting only". The layout's one callback is a
text-width measure, and inside libcontrols that goes away: libcontrols
measures with the faces it is given.

The owner asked for it "feature based with cmark as a further feature":

- **`markdown`** turns the module on with libcontrols' own minimal parser
  and no new dependency. That is enough for bookmark pills and simple text.
- **`cmark`** adds pulldown-cmark for full CommonMark blocks: headings,
  lists, code blocks, quotes and links.

## 2. Decisions

1. **Features, named `markdown` and `cmark`.**
   ```toml
   [features]
   markdown = []
   cmark = ["markdown", "dep:pulldown-cmark"]

   [dependencies]
   pulldown-cmark = { version = "0.13", default-features = false, optional = true }
   ```
   - **Why `cmark`:** it is the owner's word, it is short, and Cargo names
     no implicit feature after the dependency (because of `dep:`). The
     other option was `markdown-cmark`, which says which module it extends
     but is longer for the same meaning.
   - **No default features.** A plain build of libcontrols compiles
     neither the module nor pulldown-cmark. flankr enables `cmark`
     (open question 1).
   - This is a departure from the spec's direction that core parts are not
     behind features, made at the owner's word. Markdown is not core: no
     other control needs it. The `Scrollbar` stays unconditional.
2. **What exists in each build.**
   - **No `markdown`:** no `markdown` module, no `MarkdownView`. The theme
     types `MarkdownTheme`, `MarkdownOptions` and `MarkdownResolved`, and
     `ControlTheme::markdown`, are still built. They are plain data, and
     keeping them means `ControlTheme`'s shape does not depend on features
     (a cfg'd field would break callers' struct literals when another crate
     in the graph turns the feature on). `BookmarkText::Markdown` is drawn
     as plain text, as written (Scrollbar plan, decision 6).
   - **`markdown` without `cmark`:** the module is built and documents are
     parsed by the minimal parser (decision 4): paragraphs with inline
     formatting and bare links. A `# Title` line shows as the text
     "# Title", a list line as written, and a fence as its text with its
     lines joined. Nothing
     is lost, and nothing is styled beyond inline. `score_markdown` and
     `is_markdown` are not built, because they use pulldown-cmark.
   - **`cmark`:** documents are parsed by pulldown-cmark exactly as
     flankr-markdown parses them today (decision 5). `score_markdown`,
     `is_markdown`, `MarkdownScore` and `DEFAULT_MARKDOWN_THRESHOLD_PCT`
     come with it.
   - The inline entry point (decision 6) is the minimal parser in both
     builds, so a bookmark looks the same with or without `cmark`.
3. **libcontrols' own model**, which both parsers produce and the layout
   wraps. It is the moved code's internal `Block`/piece form made public
   and `#[non_exhaustive]`:
   - `MdStyle { Regular, Bold, Italic, BoldItalic, Code }`;
   - `MdRun { text, style, link: Option<usize>, dim: bool }`;
   - `MdBlock { runs, indent_level: list depth, quote: u8, marker:
     Option<String>, pre: bool, gap: f32 }`;
   - `MdDocument { blocks, links: Vec<String> }`.
   A caller may build an `MdDocument` itself (a diff header, a status line)
   and lay it out without parsing.
4. **The minimal parser** (`src/markdown/inline.rs`, `markdown` only):
   - **Blocks:** split at blank lines into paragraphs. Single newlines
     inside a paragraph become spaces, as CommonMark's soft breaks are
     drawn.
   - **Inline:** `***x***` bold italic, `**x**` and `__x__` bold, `*x*` and
     `_x_` italic (an `_` inside a word is literal), `` `x` `` code (no
     formatting inside), and backslash escapes of ``\ * _ ` ``.
   - **Links:** bare `http://` and `https://` URLs, trimmed of trailing
     punctuation as flankr-markdown trims them. `[text](url)` is literal.
   - **Unmatched markers** are literal text.
   It is about 150 lines, with no dependency.
5. **The cmark parser** (`src/markdown/cmark.rs`, `cmark` only) is
   flankr-markdown's `Builder` moved unchanged, emitting `MdDocument`.
   - **Kept:** headings as bold; block quotes dim and indented by four
     spaces' width a level; code blocks in `Code` keeping their lines; list
     items with "• " or "N. " and a hanging indent; emphasis, strong, code
     spans; links and bare URLs collected in order; `BLOCK_GAP` 0.5 and
     `ITEM_GAP` 0.15 (now theme values, decision 8).
   - **Basic formatting only, as the owner asked:** pulldown-cmark is
     created with no extensions, so tables, strikethrough and task lists
     (GFM) stay literal text. Raw HTML is shown as its literal text. An
     image is not drawn: its alt text is shown, linked to its URL, as now.
6. **The inline-only entry point**, for bookmark pills and one-line text:
   ```rust
   pub fn parse_inline(text: &str) -> Vec<MdRun>; // one line, no blocks
   ```
   It treats newlines as spaces and recognises only bold, italic, bold
   italic and code. Its bare-URL runs are plain text, because a pill is not
   a link. The Scrollbar draws a `Markdown` bookmark's runs with
   `MarkdownFaces` when given, else in the context's face (Scrollbar plan,
   decision 6).
7. **No measure callback.** Layout measures with a `MarkdownMeasure`, built
   from the faces and the size:
   ```rust
   pub struct MarkdownMeasure<'a> { /* faces, px, code_cell_em */ }
   impl<'a> MarkdownMeasure<'a> {
       pub fn new(faces: Option<&'a MarkdownFaces>, px: f32, theme: &MarkdownResolved) -> Self;
       pub fn width(&self, text: &str, style: MdStyle) -> f32;
   }
   pub fn lay_out(doc: &MdDocument, width: f32, m: &MarkdownMeasure) -> MdLaid;
   pub fn parse(md: &str) -> MdDocument; // minimal, or cmark with the feature
   ```
   - With faces: prose is measured by each face's shaper. Code is
     `code_cell_em` (0.6) a character in the terminal face.
   - Without faces: `0.55` a character, which is `DrawContext::text_advance`
     without a shaper, as the app measures today.
   - A caller that lays out without a view (the agent view, the settings
     screen) uses `parse` and `lay_out`, or `MarkdownMeasure::width`, with
     no closure.
   - `MdLaid { lines: Vec<MdLine>, links }` and `MdLine { runs, indent,
     gap_before }` are the moved `Laid` and `Line`.
8. **Theme per spec 8.5.** `ControlTheme::markdown: MarkdownTheme`, which
   replaces the app's `theme()` colours and its constants:
   - `text_color: Option<[f32; 4]>`: `None` is the theme's `text_color`;
   - `link_color: Option<[f32; 4]>`: `None` is `accent_color`;
   - `dim_color: Option<[f32; 4]>`: `None` is `muted_text_color`, for
     quotes;
   - `code_color: Option<[f32; 4]>`: `None` is the text colour;
   - `font_size: Option<f32>`: `None` is `default_font_size`;
   - `line_height` 1.35 (times the size);
   - `block_gap` 0.5 and `item_gap` 0.15 (in lines);
   - `quote_indent_chars` 4;
   - `underline_thickness` 0.07 (times the size, at least 1);
   - `underline_at` 0.92 (of the line);
   - `link_tap_above` 0.15 and `link_tap_height` 1.3 (in lines);
   - `code_cell_em` 0.6.
   `MarkdownOptions` overrides each value and sets the per-instance
   `centered: bool`, which replaces the app's `center()` call (decision 9).
   `with_theme` resolves into `MarkdownResolved`.
9. **`MarkdownView`, moved from the app.**
   - **Building:** `MarkdownView::new(md, origin, width, options)` and
     `.with_faces(Arc<MarkdownFaces>)`. It parses and lays out on
     construction, as the app's view does. `set_text(md)` and
     `set_width(w)` lay out again.
   - **Moved unchanged:** `height`, `rect`, `px`, `source`, `text`,
     `links`, `move_to`, `link_at`, `link_rect`.
   - **Centring:** `center()` is kept, and `options.centered(true)` does
     the same at every layout.
   - **Drawing:** runs in their face; links in the link colour and
     underlined; quotes in the dim colour; code in the code colour, in the
     terminal face when faces are given.
   - **Input:** `Click` on a link answers `ControlAction::Activated { part:
     ControlPart::Custom(index) }`. Nothing else is handled, and there is
     no callback, as now.
   - **Construction-time measuring:** the view is measured at
     construction, by its faces or the 0.55 estimate. With no faces, a
     `DrawContext` shaper used at draw may differ from the estimate. That
     is the app's behaviour today, and is kept (open question 3).
10. **`MarkdownFaces`**, moved from the app's `Faces` (with `Face`,
    `CodeFace` and `metrics`):
    - prose faces: `regular`, `bold`, `italic`, each an `AtlasId`, a
      `FontAtlas` (metrics only) and a `TextShaper`. `BoldItalic` falls back
      to `bold`;
    - `code`: a `TerminalShaper` and its text and symbol atlases;
    - `px_range`.
    `MarkdownFaces` is in the `markdown` feature. The Scrollbar's
    `ScrollbarState::set_faces(Option<Arc<MarkdownFaces>>)` is added with
    it, behind the same feature.
11. **Version.** Additive (section 6). It is the release after the
    Scrollbar's, 0.10.6 as numbered here. If it lands first it takes the
    lower number, and the Markdown bookmark drawing (Scrollbar plan,
    decision 6) is built in whichever of the two lands second.

## 3. Interfaces

`src/markdown/` (`mod.rs`, `model.rs`, `inline.rs`, `layout.rs`,
`view.rs`, `faces.rs`, and `cmark.rs` and `detect.rs` under `cmark`),
behind `#[cfg(feature = "markdown")]`. `MarkdownTheme`, `MarkdownOptions`
and `MarkdownResolved` are in `src/theme.rs`, unconditional. Everything
public is re-exported from the crate root under the same cfg.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MdStyle { Regular, Bold, Italic, BoldItalic, Code }

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct MdRun { pub text: String, pub style: MdStyle, pub link: Option<usize>, pub dim: bool }
impl MdRun { pub fn new(text: impl Into<String>, style: MdStyle) -> Self; }

#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdBlock {
    pub runs: Vec<MdRun>,
    pub list_depth: u8,
    pub quote: u8,
    pub marker: Option<String>, // "• " or "3. "
    pub pre: bool,              // a code block: lines kept, no wrapping at spaces
    pub gap: f32,               // lines before it
}

#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdDocument { pub blocks: Vec<MdBlock>, pub links: Vec<String> }

#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdLine { pub runs: Vec<MdRun>, pub indent: f32, pub gap_before: f32 }

#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub struct MdLaid { pub lines: Vec<MdLine>, pub links: Vec<String> }

pub fn parse(md: &str) -> MdDocument;          // decision 4, or 5 with `cmark`
pub fn parse_inline(text: &str) -> Vec<MdRun>; // decision 6
pub fn lay_out(doc: &MdDocument, width: f32, m: &MarkdownMeasure) -> MdLaid;

#[cfg(feature = "cmark")] pub struct MarkdownScore { /* moved */ }
#[cfg(feature = "cmark")] pub fn score_markdown(text: &str) -> MarkdownScore;
#[cfg(feature = "cmark")] pub fn is_markdown(text: &str, threshold_pct: f32) -> bool;
#[cfg(feature = "cmark")] pub const DEFAULT_MARKDOWN_THRESHOLD_PCT: f32 = 15.0;

pub struct MarkdownView { /* source, laid, placed, rect, options, faces */ }
impl MarkdownView {
    pub fn new(md: &str, origin: [f32; 2], width: f32, options: MarkdownOptions) -> Self;
    #[must_use] pub fn with_faces(self, faces: Arc<MarkdownFaces>) -> Self;
    pub fn set_text(&mut self, md: &str);
    pub fn set_width(&mut self, width: f32);
    pub fn height(&self) -> f32;
    pub fn rect(&self) -> Rect;
    pub fn px(&self) -> f32;
    pub fn source(&self) -> &str;
    pub fn text(&self) -> String;
    pub fn links(&self) -> &[String];
    pub fn move_to(&mut self, x: f32, y: f32);
    pub fn center(&mut self);
    pub fn link_at(&self, pos: [f32; 2]) -> Option<usize>;
    pub fn link_rect(&self, index: usize) -> Option<Rect>;
}
// impl DrawControl + HostedControl for MarkdownView (decision 9)

// src/theme.rs, unconditional:
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MarkdownTheme { /* decision 8's fields and defaults */ }
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct MarkdownOptions { /* an Option per field; centered: bool */ }
impl MarkdownOptions { /* a setter per field; centered(bool) */
    pub fn with_theme(&self, theme: &MarkdownTheme) -> MarkdownResolved; }
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct MarkdownResolved { /* plain values; colours stay Option, read
    against ControlTheme at draw */ }
// ControlTheme, appended: pub markdown: MarkdownTheme,
```

Because `MarkdownResolved` keeps `None` colours to read against
`ControlTheme`, the view resolves at draw time from `DrawContext.theme`, as
spec 8.5 item 3 says. `MarkdownView::new` lays out with
`ControlTheme::default().markdown` resolved against its options, and
`set_text` and `set_width` do the same. A theme whose `markdown` sizes
differ reaches the layout at the next of those calls (open question 3).

## 4. Steps

Tests: unit tests in the module for the parsers and the layout, and
router-level tests in `tests/markdown.rs` (a `Router` with a `MarkdownView`
node, a `DrawContext` without a shaper, and counters over the `DrawList`),
as the PopupMenu plan's. A CI-style check builds the crate three ways: no
features, `--features markdown` and `--features cmark`.

1. **Theme and features.** `MarkdownTheme`, `MarkdownOptions`,
   `MarkdownResolved` and `ControlTheme::markdown` (unconditional), and the
   two features in `Cargo.toml` with the optional dependency.
   **Done when:**
   - `cargo test -p libcontrols` passes with no features, and the build
     tree has no pulldown-cmark;
   - a unit test resolves `.link_color(..)` over a theme with `block_gap`
     0.75 and gets both;
   - the defaults equal section 2's numbers.
2. **The model, the minimal parser, the layout** (`markdown`). `MdStyle`,
   `MdRun`, `MdBlock`, `MdDocument`, `MdLine`, `MdLaid`, `parse` (minimal),
   `parse_inline`, `MarkdownMeasure`, `lay_out` (flankr-markdown's `wrap`,
   moved).
   **Done when:** unit tests show:
   - "Some **bold**, *italic*, `code` and https://example.com." gives the
     four styles and one link;
   - `***x***` is bold italic, `snake_case_name` is literal, and an
     unmatched `**` is literal;
   - two paragraphs give one block gap of 0.5 lines;
   - "# Title" is literal text;
   - `parse_inline` makes the URL plain;
   - words wrap at spaces and a long word breaks by character, as in
     flankr-markdown's tests (moved);
   - the build passes with `--features markdown` and no pulldown-cmark.
3. **The cmark parser and detection** (`cmark`). `cmark.rs` (the moved
   `Builder`), and `detect.rs` (`score_markdown`, `is_markdown`).
   **Done when:** flankr-markdown's tests, moved, pass unchanged in
   meaning: headings bold, quotes dim, code blocks keep their lines, HTML
   literal, the image as linked alt text, list hanging indents, links in
   order with bare URLs, empty text empty, and the five detection tests. A
   GFM table stays literal.
4. **The view and faces** (`markdown`). `MarkdownFaces`, `Face`,
   `CodeFace`, `MarkdownView` (decision 9), and `ScrollbarState::set_faces`
   if the Scrollbar is already in.
   **Done when:** router tests show:
   - a view's text and height for a fixture;
   - a `Click` on a link answers `Activated { part: Custom(0) }`, and a
     click off it nothing;
   - links are drawn in `accent_color` with one underline box each, and
     quotes in `muted_text_color` (with `cmark`);
   - overriding `link_color` reaches the instances;
   - `centered(true)` centres each line;
   - `move_to` moves every run;
   - with the Scrollbar in, a `Markdown` bookmark draws `**Landed**` as
     "Landed", brighter than plain pills, with the markers gone.
5. **Spec, README, version.** The spec amendment of section 5, the README's
   row moved to the plan's new status and a short "Markdown" paragraph
   naming the features, `Cargo.toml` 0.10.6, the `VERSIONS.md` entry of
   section 6, and the tag.
   **Done when:** `cargo fmt -p libcontrols -- --check`, and `cargo clippy
   -p libcontrols --all-targets` and `cargo test -p libcontrols` for each
   of no features, `markdown` and `cmark`, pass with no new warnings; the
   lead commits and tags `v0.10.6`.

## 5. The spec amendment (drafted; appended when step 5 lands)

```markdown
## Amendment — <date from the clock>: Markdown, behind the `markdown` and `cmark` features (sections 3, 6, 8, 10, 11)

Plan `docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md`, release 0.10.6
(`v0.10.6`). This adds section 6.19 and extends 3, 8, 10 and 11; it
replaces no earlier text except the version in 10 and the test counts in
11.

1. **3:** two cargo features, none on by default: `markdown` (the
   `markdown` module, no new dependency) and `cmark` (`markdown` plus
   pulldown-cmark 0.13, default features off). The theme types and the
   Scrollbar are built in every configuration.
2. **6.19 Markdown (`src/markdown/`):** `parse` (the minimal parser without
   `cmark`: paragraphs, inline bold, italic, code, bare links; CommonMark
   blocks with it, no GFM, HTML literal, images as linked alt text),
   `parse_inline`, the model (`MdStyle`, `MdRun`, `MdBlock`, `MdDocument`,
   `MdLine`, `MdLaid`), `MarkdownMeasure`, `lay_out`, `MarkdownFaces`,
   `MarkdownView` (links answer `Activated { part: Custom(index) }`), and
   with `cmark` `score_markdown` and `is_markdown`.
3. **8, theme:** `ControlTheme::markdown: MarkdownTheme` with the plan's
   defaults, `MarkdownOptions`, `MarkdownResolved`, built to 8.5.
4. **10:** the crate is 0.10.6. The model types, `MdStyle`, `MarkdownTheme`,
   `MarkdownOptions` and `MarkdownResolved` are `#[non_exhaustive]`.
5. **11:** `tests/markdown.rs` adds <n> router tests; the crate is tested
   with no features, `markdown` and `cmark`.
```

## 6. Versioning

0.10.6, a patch release. Every public change is additive:

- **New features** with no default. Cargo features only add, and a build
  without them is the same as 0.10.5's apart from the theme types.
- **A new optional dependency**, used only under `cmark`.
- **New types and functions** in a new module, and new `#[non_exhaustive]`
  theme types.
- **`ControlTheme::markdown` appended**, unconditional, additive by the
  owner's ruling (spec 8.5 item 4).
- **`ScrollbarState::set_faces` added**, under `markdown`.
- **Feature unification:** if some crate in a build turns on `cmark`, a
  `MarkdownView` in another crate of that build parses with CommonMark
  instead of the minimal parser. The texts that differ are block syntax
  the minimal parser shows literally, so this only adds formatting. It is
  documented in the module's docs, and is not a change between releases.

The release follows the Scrollbar's (0.10.5) here. If this lands first it
is the lower number, and the numbers follow release order (with the radio
plan's 0.10.4 likewise).

Draft `VERSIONS.md` entry (the short commit filled in when committed):

```markdown
## 0.10.6 — <date> (<short commit>, tag `v0.10.6`)

Markdown (docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md). Additive: two
new features, none on by default, a new optional dependency, a new module,
and `ControlTheme::markdown` appended.

- Added: features `markdown` and `cmark` (pulldown-cmark 0.13, default
  features off).
- Added (`markdown`): `parse`, `parse_inline`, `lay_out`,
  `MarkdownMeasure`, `MdStyle`, `MdRun`, `MdBlock`, `MdDocument`, `MdLine`,
  `MdLaid`, `MarkdownFaces`, `Face`, `CodeFace`, `MarkdownView`;
  `ScrollbarState::set_faces`.
- Added (`cmark`): `score_markdown`, `is_markdown`, `MarkdownScore`,
  `DEFAULT_MARKDOWN_THRESHOLD_PCT`.
- Added: `MarkdownTheme`, `MarkdownOptions`, `MarkdownResolved`;
  `ControlTheme::markdown`.
```

Draft `CHANGES.md` addendum (appended, timestamped from the clock, when the
owner approves this plan and implementation starts):

```markdown
## <date from the clock> — Markdown behind features (plan 20261028_09_MARKDOWN_VIEW_PLAN)

New work: flankr's Markdown layout (flankr-markdown) and its `MarkdownView`
moved into libcontrols, basic formatting only, behind a `markdown` feature
with a minimal parser and a `cmark` feature adding pulldown-cmark, at the
owner's word ("we could move the markdown render as well, if it doesn't
have heavy callbacks"; "feature based with cmark as a further feature").
Plan docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md; additive, 0.10.6
(tag `v0.10.6`).
```

## 7. Later (flankr, not this plan)

flankr then drops flankr-markdown and `apps/companion/src/markdown_view.rs`
for libcontrols' module, by its own flankr plan. That plan:

- enables `cmark`;
- maps `Style` to `MdStyle` and `Faces` to `MarkdownFaces`;
- sets `ControlTheme::markdown`'s colours from its `theme()`;
- moves the agent view's `lay_out` calls to `parse` and `lay_out`.

Once flankr's copy is removed, its markdown renderer plan
(docs/design/20261024_04_MARKDOWN_RENDERER_PLAN.md) is set to `Retired (by
<that plan>)`, with the removing commit and the last commit that has the
code. The file preview (flankr plan 20261028_05) uses `MarkdownView` and
the bookmarks' Markdown with `cmark`.

## 8. Open questions

1. **No default features.** Is that right, or should `markdown` (no
   dependency) be on by default so bookmarks format out of the box?
2. **Feature names.** `cmark` or `markdown-cmark`? This plan recommends
   `cmark` (decision 1).
3. **When the view measures.** It lays out at construction with faces or
   the 0.55 estimate, as the app's view does. Should it lay out instead
   at its first draw, with the context's shaper and theme, at the cost of
   `height()` not being known before then?
4. **More in the minimal parser.** Should it also take `[text](url)` links
   and `#` headings, or stay at inline plus paragraphs as the owner's
   "basic" suggests?
5. **Detection.** `score_markdown` and `is_markdown` use pulldown-cmark, so
   they come only with `cmark`. Is that acceptable, or are they wanted in
   libcontrols at all, rather than staying in flankr?

## Amendment — 2026-10-28 23:13 EDT: approved; built after the Scrollbar

The owner approved this plan on 2026-10-28 ("approve and build", given for
this plan and plan 20261028_08), after "feature based with cmark as a
further feature" and "but yes, it should be basic formatting only". It is
built after the Scrollbar's first release, so its version is **0.10.5**
(release order), not the drafted 0.10.6. The open questions take the
plan's recommendations: no default features, the feature named `cmark`,
measuring at construction as the app does, the minimal parser limited to
paragraphs, emphasis, code, escapes and bare links, and the detection
functions moved with the parser. The status stays `Draft` until the
first implementing commit (Rule 1.5).
