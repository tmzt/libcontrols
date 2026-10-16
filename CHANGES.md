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

## 2026-10-16 00:23 EDT — Modals and input filters (supersedes Modal layer)

Plan: [Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md)
(`Draft`), superseding [Modal layer](docs/design/20261015_01_MODAL_PLAN.md).

- Adds an input filter stack: `HostedControl::intercept`, a defaulted method
  asked last-added control first before normal routing; a control that
  answers `Capture` receives the event alone. With every control passing,
  routing is 0.2.0's.
- Modals are rebuilt on it: a `ModalStack` hosted control, added last,
  captures every event while its `ModalState` holds any modal, so the base
  UI is level 0 and gets nothing, modifiers included. The host-level modal
  API of the old plan is dropped.
- Answers the old plan's open questions: `Modal::with_hold_open` and
  `ModalEvent::DismissRefused` veto user dismissals; Escape and Back
  dismiss even with an `Edit` focused; scrolling is a later plan; the
  per-instance fade stays in libcontrols.
- `Panel` uses the filter: hover inside panels follows plain moves (a fix),
  and opt-in `with_focus_traversal` lets Tab and Escape traverse children.
- Still breaking (0.3.0) only through `Key::Back` and `#[non_exhaustive]`
  `Key`.

## 2026-10-16 00:58 EDT — Input routing and filters; Modal stack (supersede Modals and input filters)

Plans: [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md)
(`Draft`) and [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md)
(`Draft`, depends on the first), together superseding
[Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md).

- The owner rejected `HostedControl::intercept` (every control asked on
  every event, inside a GPU render loop) and asked for a clean split into a
  filters plan and a modal plan. Nothing of the old plan was implemented.
- Routing contract (plan 02): an app-owned `InputFilter` stack sees every
  event first and answers `Pass`, `Consumed(action)` or `Redirect(key)`;
  controls are never asked and are never filters themselves. Pointer events
  and gestures route by area with a press grab; keys route by focus; Tab,
  BackTab and Escape belong to the focus manager; `Key::Back` is an
  ordinary key.
- Focus changes are events: `FocusEvent::{Gained, Lost}` through
  `HostedControl::handle_focus`; `accepts_focus` lets a control refuse Tab.
  Hover is enter and leave: at most two `set_hover` calls per pointer
  event, with `hovered_key` carried across rebuilds like focus and drag.
- Modifiers are no longer broadcast: `HostedControl::set_modifiers` is
  removed and every handler takes an `EventContext { part, modifiers,
  press_modifiers }`, so modified drags work without a modifier event.
  Only filters see `InputEvent::Modifiers`.
- `Symbol` (one process-wide interner, `sym!` interning once per call
  site) replaces `String` in `ControlKey::Named`, control names,
  `find_by_id` and `EventResponse::target_id`; tinytrowel's `"speed"` lines
  compile unchanged.
- Modals (plan 03): `ModalState` (stack of four, per-entry transition,
  focus, drag, hover, events via `take_events`), a `ModalFilter` created
  once from the state that answers `Redirect` to a hosted `ModalStack` by
  name while any modal exists, per-build `Modal`s with app-drawn frames,
  backdrops, dismiss rules and a hold-open veto; Escape and Back dismiss
  the top even with an `Edit` focused. `ControlHost::draw_controls` and
  `draw_control_overlays` are added there for clipping.
- **Breaking, 0.3.0**, cut once in plan 03's last step with the
  `VERSIONS.md` entry and the `v0.3.0` tag; plan 02 bumps nothing.
- Deferred to a future "focus manager: nested containers" plan: symbol
  paths through nested hosts, Tab order across nesting, `Panel`'s hover
  defects and child tooltips in the first pass.
