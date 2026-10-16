# CHANGES

Planned new work, feature plans and architectural changes, appended in order
(AGENTS.md, Rule 0). Entries are never edited.

## 2026-10-15 23:25 EDT — Modal layer

Plan: [Modal layer](docs/design/20261015_01_MODAL_PLAN.md) (`Draft`).

- Adds a modal layer to libcontrols for Noble's Keep, whose UI is limited to
  modals opened from on-screen icons: a persistent `ModalState` (phases,
  transition progress, `ModalEvent`s), a per-build `Modal` with an
  app-drawn `ModalFrame`, backdrop and dismiss rules, and a modal layer in
  `ControlHost` that captures all input and draws above every overlay.
- Additive only: no existing public type changes; tinytrowel keeps compiling.

## 2026-10-15 23:31 EDT — Key::Back and a modal stack (breaking: 0.3.0)

Plan: [Modal layer](docs/design/20261015_01_MODAL_PLAN.md), amendment
"2026-10-15 23:31 EDT: Key::Back and a modal stack".

- **Breaking change:** `Key` gains a `Back` variant for the platform back
  action (Android Back, browser back) and becomes `#[non_exhaustive]`.
  `Key` was exhaustive, so a downstream exhaustive `match` on it no longer
  compiles; the version goes to 0.3.0 instead of 0.2.1. Shells send back as
  `Key::Back`, no longer as `Escape`. Both dismiss the top modal, reported
  as `DismissReason::Back` and `DismissReason::Escape`.
- `ModalState` holds a stack of up to four modals instead of one: push,
  pop, replace-top, dismiss-all, per-entry transitions; only the top takes
  input and is dismissed, and lower modals stay drawn under its backdrop.
