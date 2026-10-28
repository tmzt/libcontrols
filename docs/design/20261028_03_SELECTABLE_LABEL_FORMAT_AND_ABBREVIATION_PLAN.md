---
title: SelectableLabel for any Display type, with format_fn and an opt-in abbreviation
date: 2026-10-28
status: Draft
covers:
  - "`SelectableLabel<T>` drawing for every `T: Display`, not only
    `&'static str`"
  - "Its shown text made by `format_fn` (or `Display`), then, only when
    enabled by a setter, split by the `\"XX - Name\"` convention into an
    abbreviation and a tooltip"
  - "Whether `Radio` and `Options` keep the convention on by default (open
    question 1)"
sources:
  - "The owner (2026-10-28), answering the library spec's open question 4:
    \"chain them, but split should be enabled via setter, it was added for
    a minimalist ui but this is a common UI library\""
  - "docs/design/20261028_02_LIBCONTROLS_SPEC.md, section 6.6
    (SelectableLabel), section 12 (its `HostedControl` for every `T` but
    `DrawControl` only for `&'static str`) and open question 4"
  - "src/selectable_label.rs: `format_fn: Option<fn(&T) -> String>` (:86),
    `with_format_fn` (:167), `display_string` (:270-281), `impl
    DrawControl for SelectableLabel<&'static str>` (:284), `impl<T:
    'static> HostedControl for SelectableLabel<T>` (:379)"
  - "src/tooltip.rs: `split_label` (:13-25), `draw_tooltip`,
    `TooltipStyle`; src/radio.rs and src/options.rs: `abbreviate: bool`
    (default `true`), `with_abbreviate`, their use of `split_label`
    (radio.rs:298, options.rs:417)"
related:
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; sections 6.6, 6.7, 6.8 and 12 are amended when this is implemented)
---

# SelectableLabel for any Display type, with format_fn and an opt-in abbreviation

## 1. Goal

`SelectableLabel<T>` is generic, but it draws only when `T` is `&'static
str`, so its `format_fn` is never used on a drawn label and a label written
`"XX - Name"` is shown as written. After this plan, a `SelectableLabel` of
any `T: Display` draws: its text comes from `format_fn` when given, else
from `Display`, and only when the app turns it on is that text split into an
abbreviation drawn in the label and the name shown as a tooltip, as `Radio`
and `Options` do. The split stays a setting, off by default: it was added
for one minimalist UI, and a general library draws text as written unless
asked.

## 2. Decisions

1. **One draw for every `T: Display`.** `impl<T: std::fmt::Display +
   'static> DrawControl for SelectableLabel<T>` replaces the `&'static str`
   impl (which `&'static str` still satisfies, so existing users are
   unchanged). The centre text is `display_string()` (`format_fn`, else
   `Display`).
2. **The split is opt-in.** `SelectableLabel` gains `pub abbreviate: bool`,
   default `false`, and `with_abbreviate(bool)`, the same name and meaning
   as `Radio`'s and `Options`'. Off: the text is drawn as written, `"XX -
   Name"` included. On: `split_label(&text)` is applied to the formatted
   text; the abbreviation is drawn, and the name is the tooltip shown on
   hover (with `tooltip_style`, a new `Option<TooltipStyle>` field and
   setter, as Radio has). A text without the separator is drawn as written
   either way.
3. **The chain.** `format_fn`/`Display` first, then the split: a
   `SelectableLabel<Mode>` whose `format_fn` returns `"CS - Clear sky"`
   draws "CS" with the tooltip "Clear sky" when abbreviation is on.
4. **Versions.** A blanket impl replacing a narrower one, an appended
   field and a new setter are additive: 0.9.x to the next patch, by the
   owner's ruling ("appending to a struct is additive").
5. **The spec.** The commit that implements this appends an amendment to
   the living spec (`20261028_02`): section 6.6 for the new draw and
   setter, and the section 12 gap closed.

## 3. Steps

1. **Draw for every `T: Display`** (decision 1), with the field
   `abbreviate`, `with_abbreviate`, `tooltip_style`/`with_tooltip_style`,
   and the split on the formatted text (decisions 2-3).
   **Done when:** tests show a `SelectableLabel<&str>` drawing exactly as
   before; a `SelectableLabel<u32>` with a `format_fn` drawing the
   function's text; `"XX - Name"` drawn whole with abbreviation off, and as
   "XX" with the tooltip "Name" on hover with it on; a text without the
   separator drawn as written either way.
2. **Release** with its `VERSIONS.md` entry and tag, and the spec
   amendment (decision 5).
   **Done when:** `cargo fmt --check`, `cargo clippy --all-targets` and
   `cargo test` pass and the tag is made.

## 4. Open questions for the owner

1. **`Radio` and `Options` default to `abbreviate: true`** today. Should
   they follow the same rule and default to `false`, so every control draws
   text as written unless the app turns the split on? That changes what an
   existing caller sees, so it is breaking (0.10.0), and the UI that relies
   on it adds `.with_abbreviate(true)`. Or do they keep `true`, with only
   `SelectableLabel` off by default?

## Amendment — 2026-10-28 12:30 EDT: Radio and Options draw labels as written by default; a breaking 0.10.0

The owner (2026-10-28), answering open question 1: "yes, breaking is okay,
it's not an expected behsvior of a UI library, just document it". Where
this disagrees with the body, it wins.

1. **`Radio` and `Options` default to `abbreviate: false`**, like
   `SelectableLabel`: a label is drawn as written, `"XX - Name"`
   included, with no tooltip, unless the app calls
   `.with_abbreviate(true)`. With it on they behave as today. Their field
   docs ("the default") are rewritten to say the split is opt-in.
2. **This is breaking** (what an existing caller sees changes), so the
   release is **0.10.0**, not the patch of decision 4. The `VERSIONS.md`
   entry marks it **Breaking**, says what changed and gives the one-line
   migration: add `.with_abbreviate(true)` to every `Radio` and
   `Options` that relied on the abbreviation. The README's "Labels and
   tooltips" section is rewritten the same way: the convention is opt-in
   per control.
3. **Step 1** also flips the two defaults, with tests: a `Radio` and an
   `Options` drawing `"XX - Name"` whole by default and as "XX" with the
   tooltip with `.with_abbreviate(true)`; their existing abbreviation tests
   set it explicitly. **Step 2** releases 0.10.0 with the tag `v0.10.0`
   (the repo's tag form), and the living spec's amendment also updates
   sections 6.7 and 6.8.

The plan stays `Draft` until the owner approves it.
