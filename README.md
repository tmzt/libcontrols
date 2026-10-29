# libcontrols

UI controls drawn as MSDF primitives, owned by a persistent router.

`libcontrols` provides a set of reusable UI controls (labels, buttons, sliders, radios, option grids, panels, dialogs) that render directly into [`libmsdf`](https://github.com/tmzt/libmsdf) draw lists. A `Router` owns the controls as a tree for as long as the app keeps it, routes input to them, and draws them every frame; the app edits the tree only when its UI changes.

## Controls

- **Label** — Text label with optional background and active focus ring
- **Button** — Clickable button with press state and momentary click tracking
- **Slider** — Horizontal slider with configurable thumb styling
- **SliderNumeric** — Slider with integrated numeric value display and editing
- **ArrowLabel** — Label with left/right navigation arrows for discrete options
- **SelectableLabel** — Labeled option list with arrow navigation
- **Radio** — Mutually exclusive cell strip for modal selection
- **Options** — Packed grid of independent toggle cells
- **Panel** — Container with optional header, background, and border
- **Edit** — Text input field with selection and formatting
- **EditableLabel** — A label with a pencil that edits it in place: a field with a check and an optional X (clear)
- **PopupMenu** — A menu of actions anchored to a rect (a ⋮), opened by a call and shown as a layer of the modal stack; an outside tap, Escape or Back closes it
- **ScrollPane** — A viewport onto children laid out on a larger plane, scrolled by touch, wheel, trackpad and keys
- **Scrollbar** — A thin proportional thumb with a band that grabs and scrubs, shown on scroll or always, with bookmarks drawn while it is held; on a pane or over the app's own scroller

## Labels and tooltips

Labels are drawn as written, `"XX - Name"` included. Splitting a label into an
abbreviation and a tooltip is opt-in per control: `Button`, `Radio`, `Options` and
`SelectableLabel` take `.with_abbreviate(true)` to draw the two-letter
abbreviation in the cell and show the name as a tooltip on hover (styled with
`.with_tooltip_style`), for a row too narrow for whole words. A label without
the `" - "` separator is drawn as written either way. `SelectableLabel<T>` draws
for any `T: Display`: its text comes from `.with_format_fn` when given, else
from `Display`, and the split, when on, applies to that formatted text.

## Usage

```rust
// Once, when the UI is created:
let mut router = Router::new();
let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 300.0, 200.0)).with_title("AUDIO"));
let volume = router.add(Some(panel), Slider::new(0.5, 0.0, 1.0, Rect::new(10.0, 40.0, 280.0, 20.0)));
let mute = Button::new("Mute", Rect::new(10.0, 80.0, 80.0, 30.0));
let mute_state = mute.state.clone();               // values live in Arc state
let mute = router.add(Some(panel), mute);

// For every input event:
let response = router.deliver_event(event);
if response.target == Some(mute) && mute_state.take_click() { /* toggle */ }

// When the layout changes:
router.get_mut::<Slider>(volume).unwrap().rect = Rect::new(10.0, 40.0, 200.0, 20.0);

// Every frame:
router.render(&mut draw_list, &draw_context);
```

Nodes are named by `NodeId`s (written `@17`), which the app keeps in the
struct that owns its UI. A node may also get an alias unique within its router
(`add_named`), found again with `find("ok")`; aliases starting with `@` or
`libcontrols/` are reserved.

## Input routing

Each event the router is given passes through chains, in order:

1. **The global filter chain**, a `FilterStack` given to the root router: app-wide
   rules such as accelerators. Rules are asked last pushed first and may pass
   an event on, consume it, or redirect it to one node.
2. **The modal stack**, if one is installed and holds a layer (below).
3. **Routing**:
   - **Pointer events and gestures** go to the innermost node whose area
     contains the pointer. A press grabs that node: until the button is
     released, moves, drags and the release go to it even off its area. A
     plain move outside a grab only updates hover.
   - **Keys** go to the focused node. Tab and BackTab step through the focus
     stops in tree order, into containers and out again; Escape clears the
     focus; those three reach no node. `Key::Back` is an ordinary key.
   - **Focus changes** arrive as `handle_focus(FocusEvent::Gained)` and `Lost`,
     only when the focus actually moves.
   - **Hover** is enter and leave along the parent chain: only the nodes the
     pointer left and the ones it entered are told.
   - **Modifiers** arrive with every event in its `EventContext`
     (`ctx.modifiers`, and `ctx.press_modifiers` for the press that started a
     drag). No control is told when Shift goes down.

Each node has declared `NodeFlags` (`FOCUSABLE`, `DISABLED`, `HIDDEN`,
`TAB_THROUGH`), changed only with `router.set_flags`, and router-owned
`NodeState` (`FOCUSED`, `FOCUS_WITHIN`, `HOVERED`, `HOVER_WITHIN`, `GRABBED`).
Both reach a control in its `DrawContext` and `EventContext`, so a button
draws its focus ring from `ctx.state`, and a panel glows while a child is
focused. A disabled subtree takes no input and is drawn at half alpha.

A `Router` is itself a control: nested in another, it is one Tab stop that
keeps Tab inside it (a focus scope), or, flagged `TAB_THROUGH`, one whose
stops join the parent's.

```rust
struct SaveShortcut;
impl InputFilter for SaveShortcut {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        match event {
            InputEvent::Keyboard(KeyboardEvent::KeyDown { key: Key::Char('s') })
                if ctx.modifiers.ctrl => Filtered::Consumed(Some(ControlAction::Custom("save".into()))),
            _ => Filtered::Pass,
        }
    }
}
let filters = FilterStack::new();
filters.add(SaveShortcut);
let router = Router::new().with_filter_stack(filters);
```

## Modals

`ModalStack::install` puts a modal stack in the root router, above every app
root. Anything that implements the `Modal` trait can be pushed on it: the
built-in `ModalDialog` (a frame, a content router and close controls), or a
self-drawn or shaped control. Each layer owns the filter it is asked through
while it is on top, so nothing beneath it receives input, and popping a layer
drops its filter with it. The standard filter blocks pointer events outside
the modal's bounds (or dismisses on an outside tap, by its `ModalOptions`) and
dismisses on Escape and Back; a shaped modal returns a `ShapedLayerFilter`
over the SDF shapes it actually draws, or over an alpha mask.
`ModalOptions::with_hold_open(true)` refuses user dismissals and reports
`ModalEvent::DismissRefused` instead, for unsaved changes. Transitions are
`Timed` (moved by `ModalState::advance`), `Driven` (the app sets progress) or
`Instant`, and a modal animates from the `LayerView` in its contexts.

```rust
// Once:
let modals = Arc::new(ModalState::new());
ModalStack::install(&mut router, modals.clone(), viewport)?;

// The settings icon pushes a dialog; its Quit button pushes a confirm over it.
let mut settings = ModalDialog::centered(viewport, [400.0, 300.0], 16.0)
    .with_options(ModalOptions::default().with_hold_open(unsaved_changes));
let quit = settings.content_mut().add(None, Button::new("Quit", quit_rect));
router.push_modal(sym!("settings"), settings)?;

let mut confirm = ModalDialog::centered(viewport, [240.0, 120.0], 12.0);
let cancel = confirm.content_mut().add(None, Button::new("Cancel", cancel_rect));
router.push_modal(sym!("confirm"), confirm.with_close_control(cancel))?;

// Every frame:
modals.advance(dt);
for event in modals.take_events() { /* Closed { id, by }, DismissRefused { .. } */ }
router.render(&mut draw_list, &draw_context);
```

## Scroll panes

A `ScrollPane` is a node whose children are laid out on a content plane, from
the pane's top-left corner at offset zero, as if the pane were as tall (or
wide) as its content. The router clips them to the viewport, moves them by the
offset, skips the ones out of view, and gives them events in their own
coordinates, so a control inside a pane works as it does anywhere else.

```rust
let place = ScrollState::new(); // keep it across rebuilds to keep the place
let list = router.add(
    None,
    ScrollPane::new(Rect::new(0.0, 0.0, 360.0, 640.0), ScrollAxis::Vertical)
        .with_state(place.clone())
        .with_overscroll(Overscroll::Fraction(0.15)),
);
for (i, name) in names.iter().enumerate() {
    router.add(Some(list), Button::new(name, Rect::new(0.0, i as f32 * 48.0, 360.0, 48.0)));
}

// Every frame:
router.advance(dt); // flings, the overscroll spring and the indicator
router.render(&mut draw_list, &draw_context);
```

- **Tap or scroll.** A press goes to the control under it. Once it moves past
  the touch slop (`ScrollTheme::touch_slop`, 8 px) along the pane's axis, the
  pane takes it: the control is told the press is up and its `Click` is
  dropped. Buttons and labels activate on the `Click`, so a scroll never
  presses them. Sliders and edits keep their drags (`HostedControl::claims_drag`).
- **Fling.** A lift while moving flings, decaying on `Router::advance`; a
  touch stops it. With `with_overscroll` (pixels or a share of the viewport),
  a finger pulls the content past an end with growing resistance and it springs
  back on release, as on iOS.
- **Wheel and trackpad** scroll the content the same way, after the control
  under the pointer (a slider takes the wheel first). Arrow keys, PageUp,
  PageDown, Home and End scroll the pane around the focused control, and Tab
  scrolls the focus into view.
- **Keeping the place.** `ScrollState` remembers the offset the user chose and
  returns to it when the content or viewport grows back; `ScrollState::sticky`
  follows an end, like a terminal, until scrolled back. When content changes
  above what is in view, what is in view stays where it is (anchoring).
- `Router::scroll_to`, `scroll_by`, `scroll_into_view`, `to_content` and
  `visible_bounds` move a pane and map points and bounds through it.

## Scrollbar and sections

`ScrollPane::with_scrollbar(ScrollbarOptions)` gives a vertical pane
flankr's agent-view scrollbar in place of the indicator: a 4 px thumb, at
least 24 long, whose 12 px band at the right edge takes a press even when
the thumb is not drawn. A press on the thumb grabs it where it touched;
elsewhere in the band the thumb jumps to centre under the finger. Dragging
scrubs the pane, and letting go leaves it there with no fling. The
children never see a press in the band. `ScrollbarVisibility` is `None`
(no thumb, no band), `OnScroll` (the default: shown while scrolling and
2 s after, then a 300 ms fade) or `Always`.

```rust
let pane = ScrollPane::new(viewport, ScrollAxis::Vertical)
    .with_scrollbar(ScrollbarOptions::default());
let index = SectionedContent::new();
index.set_sections(chunks.iter().map(|c| Section::new(SectionId(c.id), c.height)).collect());
index.add_bookmark(SectionId(3), BookmarkText::Text("Installation".into()));
pane.scrollbar().unwrap().set_sections(Some(index.clone()));
```

While the thumb is held, and 2 s after, bookmarks are drawn over the
content as translucent pills. Each sits at its section's share of the
track, centred across it. The pill nearest the thumb is brighter, and
crowded pills give way to the nearest one. A `SectionedContent` is an
index of sections: each has a stable `SectionId` and a height, and
bookmarks are added on a section's id (`add_bookmark` replaces,
`remove_bookmark`). `BookmarkText::Markdown` is drawn as written, or as
inline spans with the `markdown` feature (below).
A standalone `Scrollbar` node over a scroller the app drives itself is fed
with `ScrollbarState::set_metrics` and `set_scrolling`, and reports
`ScrollbarEvent::Grabbed`, `Scrub` and `Released` (with the section and
nearest bookmark) through `take_events`. Its numbers are
`ControlTheme::scrollbar`, overridden per scrollbar by `ScrollbarOptions`.

A pane can also draw its content as sections, for long content such as a
20 000-line file. `ScrollPane::with_sections` takes a `SectionedContent`
made with a `SectionSource`, the app's drawing: `render_section(id, cx)`
returns one section's `DrawList` in section coordinates (top-left at
`[0, 0]`, `cx.width()` wide). Any `Fn(SectionId, &SectionContext) ->
DrawList` is a source too.

```rust
let content = SectionedContent::with_source(Arc::new(
    move |id: SectionId, cx: &SectionContext| -> DrawList { file.draw_chunk(id, cx.width()) },
));
content.set_sections(chunks.iter().map(|c| Section::new(SectionId(c.id), c.height)).collect());
let pane = ScrollPane::new(viewport, ScrollAxis::Vertical)
    .with_sections(content.clone())
    .with_scrollbar(ScrollbarOptions::default());
```

The pane asks the source only for the sections in the window, and for at
most two a frame in a one-window margin either side, and keeps each
drawing per id. It places them under its clip and offset with libmsdf's
`DrawList::append_scoped`, before its children. A drawing is used again
until `invalidate(id)` or `invalidate_all()`, or until the width or theme
changes. Drawings more than three windows away are dropped, and at most
128 are kept (`set_prefetch`, `set_prefetch_per_frame`, `set_keep`,
`set_max_cached`). A section's height may be an estimate that its render
corrects with `cx.report_height(h)`, or be set with `set_height`. When a
section above the window changes height, or one is inserted or removed
there, the first section in view stays where it is. While the thumb is
held, that shift waits for the release. `Router::scroll_to_section(pane,
id)` puts a section's top at the window's top. A scrollbar on the pane
takes its bookmarks from the pane's sections.

## Markdown

Two cargo features, none on by default, add basic Markdown. `markdown`
builds the `markdown` module with libcontrols' own minimal parser and no
new dependency: paragraphs, bold, italic, code spans, backslash escapes and
bare `http(s)://` links; block syntax (`# Title`, list lines) shows as
written. `cmark` adds pulldown-cmark 0.13 (CommonMark, no GFM) for
headings, lists, code blocks, quotes and `[text](url)` links, with HTML
shown as its text and images as their alt text linked to their URL, plus
`score_markdown` and `is_markdown`.

```rust
let view = MarkdownView::new(md, [x, y], width, MarkdownOptions::default())
    .with_faces(faces.clone()); // Regular, Bold, Italic and the terminal face
let id = router.add(None, view);
// A tap on link i answers ControlAction::Activated { part: ControlPart::Custom(i) }.
```

`MarkdownView` lays out when built and at `set_text` and `set_width`,
measured by its `MarkdownFaces` or, without them, at 0.55 a character. It
draws links in the accent colour and underlined, quotes in the muted
colour. Its numbers are `ControlTheme::markdown`, overridden per view by
`MarkdownOptions` (`centered(true)` centres each line). Without a view,
`parse` and `lay_out` with a `MarkdownMeasure` give the wrapped lines.
With `markdown`, a scrollbar draws `BookmarkText::Markdown` pills through
`parse_inline` (the same in both builds): the markers gone, bold, italic
and code in the faces given by `ScrollbarState::set_faces`, or without
faces bold brighter and code over a chip.

## Building

```bash
cargo build
cargo test
cargo test --features markdown
cargo test --features cmark
```

## License

MIT License. See LICENSE file for details.

## Architectural decisions

| Document | Date | Status | Decision |
|---|---|---|---|
| [Modal layer](docs/design/20261015_01_MODAL_PLAN.md) | 2026-10-15 | Superseded (by [Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md)) | Proposed a modal stack built into `ControlHost`; replaced before implementation by an input filter hook and a `ModalStack` control, which carry its design forward. |
| [Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md) | 2026-10-16 | Superseded (by [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md) and [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md)) | Proposed `HostedControl::intercept`, a filter asked of every control on every event, with the modal stack and Panel fixes built on it; rejected before implementation because it adds per-control work to every event in a render loop, and split into a routing plan and a modal plan that carry its modal design forward. |
| [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md) | 2026-10-16 | Implemented | Fix the host's routing contract for 0.3.0: an app-owned `InputFilter` stack asked first (`Pass`, `Consumed`, `Redirect`), area routing with a grab, focus routing with `FocusEvent`s, hover as enter and leave, modifiers on every event in an `EventContext` (no `set_modifiers`), `Symbol` names and `Key::Back`; no per-control work per event; implemented and released in 0.3.0 (`8c34296`, tag `v0.3.0`). |
| [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md) | 2026-10-16 | Implemented | Build modals on plan 02's `Redirect`: a persistent `ModalState` stack of four, a `ModalFilter` registered once that traps input to a hosted `ModalStack`, app-drawn frames, hold-open veto, Escape and Back dismissal; cuts 0.3.0 in its last step; implemented and released in 0.3.0 (`8c34296`, tag `v0.3.0`). |
| [Focus manager: nested containers](docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md) | 2026-10-16 | Superseded (by [Persistent control tree and router layers](docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md)) | Proposed routing through nested containers from the top host via `with_children` and `ControlPath`; replaced before implementation by a persistent router whose tree makes containers plain nodes. |
| [Persistent control tree and router layers](docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md) | 2026-10-16 | Implemented | Replace `ControlHost` with a persistent `Router` that owns its controls as a tree and only draws per frame; focus, hover and the grab are `NodeId`s it keeps; `Panel` children are nodes; declared `NodeFlags` (changed only through the router) and router-owned `NodeState` reach controls in the draw and event contexts; a `Router` implements the routed-control traits so it is a layer; a global filter chain runs first, then the modal stack, whose layers are any `Modal` (trait; `ModalDialog` built in) each owning its filter (breaking, 0.4.0 with the libmsdf 0.3.0 move); implemented in `5567fa7`, released with the libmsdf 0.3.0 move as 0.4.0. |
| [Press feedback, after Material 3](docs/design/20261023_01_PRESS_FEEDBACK_PLAN.md) | 2026-10-23 | Draft | A transient press tracked by the router (`NodeState::PRESSED`, cancelled past an 8 px touch slop or by `Router::cancel_press`, held 100 ms and faded 150 ms by `Router::advance`), read as `DrawContext::press`: buttons draw M3's 10% pressed state layer and move down 2 px, plain-text tap targets shift colour within the same hue; breaking (0.8.0); drafted for the owner's review. |
| [Stateful scroll pane](docs/design/20261023_02_SCROLL_PANE_PLAN.md) | 2026-10-23 | Implemented | A `ScrollPane` node over a shareable `ScrollState` (offset, remembered offset, sticky end, fling): the router clips, translates (libmsdf `push_translate`) and culls its children, maps hits and events into content coordinates, and moves a drag past the touch slop from the child to the pane, cancelling its press; wheel, keys, focus into view, anchoring and an indicator; buttons and labels activate on release; optional iOS-style overscroll; breaking; implemented and released in 0.8.0 (tag `v0.8.0`). |
| [Cursor and insertion point in Edit](docs/design/20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md) | 2026-10-25 | Implemented | Insertion cursor and UTF-8 boundary tracking in `EditState`, vertical SDF cursor bar rendering, mouse click hit-testing to nearest character, and keyboard navigation/editing in `Edit`; breaking (0.9.0); implemented and released in 0.9.0 (tag `v0.9.0`). |
| [TextArea multi-line text edit control](docs/design/20261025_02_TEXT_AREA_CONTROL_PLAN.md) | 2026-10-25 | Implemented | Multi-line text edit control with soft line wrapping, cursor tracking, SDF cursor bar rendering, mouse hit-testing, and multi-line keyboard navigation and editing; additive; implemented and released in 0.9.1 (tag `v0.9.1`). |
| [Drawer, composited from offscreen layers](docs/design/20261028_01_DRAWER_PLAN.md) | 2026-10-28 | In implementation | A `Drawer` on one of four edges (Left by default) with a content router, driven by calls (`open`, `close`, `toggle`, `set_open_now`, `set_progress`, `settle`) with no gestures of its own, scrim only; while visible, main view and sheet are kept in wgpu textures redrawn only on change and each frame is one composite driven by an eased uniform `t`; at `t = 0` no texture, pass or copy; theme defaults in `ControlTheme::drawer`, overrides in `DrawerOptions`; approved, being built on the DrawerLayout parent.
| [libcontrols: the library specification](docs/design/20261028_02_LIBCONTROLS_SPEC.md) | 2026-10-28 | Implemented | Describes the library as it exists at `d53ef99` (0.9.1): a retained `Router` tree, the event model, the control traits, every control, the theme as the code reads it today (with living sections for the controls and the theme), stability and testing, written from the code so theming can be added as an amendment; The living spec of the library, kept current by amendment; AGENTS.md points to it. |
| [SelectableLabel for any Display type, with format_fn and an opt-in abbreviation](docs/design/20261028_03_SELECTABLE_LABEL_FORMAT_AND_ABBREVIATION_PLAN.md) | 2026-10-28 | Implemented | `SelectableLabel<T>` draws for every `T: Display`, its text from `format_fn` or `Display`, then split into an abbreviation and tooltip by the "XX - Name" convention only when `with_abbreviate(true)` is set; `Radio` and `Options` also draw labels as written unless `with_abbreviate(true)` (breaking, 0.10.0). Implemented in 0.10.0. |
| [DrawerLayout, the parent of a main view and a drawer](docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md) | 2026-10-28 | In implementation | A small parent with two `Router` children, the main view (fills its rect) and the drawer's content (sheet coordinates, placed by edge and size, offscreen when closed); it holds the `Drawer`, routes input (main normally, the sheet's content while visible, `Scrim` reported), forwards the drawer's calls and draws each side for the drawer's textures; approved with the drawer plan and built first.
| [EditableLabel, a label edited in place](docs/design/20261028_05_EDITABLE_LABEL_PLAN.md) | 2026-10-28 | In implementation | A one-node `EditableLabel`: a label with a pencil that switches it to an `Edit` field with a check and an optional X (clear, only with `with_clear(true)` and text); Enter confirms, Escape, Back and focus loss cancel; `EditableLabelState` with the committed text, the draft and `Started`/`Confirmed`/`Cancelled`/`Cleared` events, the soft keyboard left to the app; icons from line primitives; theme per 8.5 (`ControlTheme::editable_label`); additive (0.10.2); approved by the owner's request, being built. |
| [PopupMenu, a call-driven popup menu of actions](docs/design/20261028_06_POPUP_MENU_PLAN.md) | 2026-10-28 | In implementation | A `PopupMenu` anchored to a rect (an app's ⋮), opened and closed by calls on its shared `PopupMenuState` with no gestures of its own, shown as a backdrop-free layer of the modal stack (attached once with `Router::attach_popup_menu`) so an outside tap, Escape or Back dismisses it; actions, headings and separators with an enabled flag and a check; opens below and right-aligned, flipping above or shifting to fit; pointer and Up/Down/Enter keys; `Chosen(id)`/`Dismissed` events; theme per 8.5 (`ControlTheme::popup_menu`); additive (0.10.3); asked by flankr's plugin plan and approved with it, being built. |
| [A radio style for PopupMenu's mark column](docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md) | 2026-10-28 | Implemented | `PopupMenuOptions::marks(PopupMarkStyle::Radio)` draws each action's `checked` as a ring with a filled dot (⦿, `Some(true)`) or a small dot (⊙, `Some(false)`) from `Outline` and `Circle` primitives, since Roboto lacks the glyphs; toggles keep the check; per menu, `PopupItem` unchanged, `Check` stays the default; ring and dot sizes in `PopupMenuTheme`, colours defaulting to the label's (no accent); layout and input unchanged; additive (0.10.7, after the toggles and columns); asked by flankr's subagents plan and approved by the owner, being built. |
| [Scrollbar, sectioned content and bookmarks](docs/design/20261028_08_SCROLLBAR_PLAN.md) | 2026-10-28 | In implementation | flankr's agent-view scrollbar as a call-driven `Scrollbar` (4 px thumb, at least 24, 4 px slop, a band that grabs even when invisible, drag scrubs with no fling; `None`/`OnScroll` (2 s hold, 300 ms fade)/`Always`; bookmarks drawn while held as translucent pills, nearest brighter, crowded ones giving way), on a pane by `ScrollPane::with_scrollbar` or standalone; sectioned content drawn by the caller's `render_section(id) -> DrawList` only near the window and cached per `SectionId`, with section anchoring and `scroll_to_section`; bookmarks by `add_bookmark(id, BookmarkText)` (replaces) and `remove_bookmark`, section boundaries not being bookmarks; theme per 8.5 (`ControlTheme::scrollbar`); additive; asked by the owner for flankr's file preview and approved; the scrollbar, its bookmarks from an index of sections and the pane (steps 1-5, 8) are built as 0.10.4, and sections drawn by the pane (steps 6-7) as 0.10.8 on libmsdf 0.6.1's scoped append. |
| [MarkdownView, basic Markdown behind a feature](docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md) | 2026-10-28 | Implemented | flankr-markdown's layout and the app's `MarkdownView` moved into a `markdown` module, basic formatting only, measured by libcontrols' own faces (no callback), links by index, themed per 8.5 (`ControlTheme::markdown`, unconditional); feature `markdown` with a minimal parser (paragraphs, bold, italic, code, escapes, bare links) and feature `cmark` adding pulldown-cmark 0.13 for CommonMark blocks, none by default; `parse_inline` draws `BookmarkText::Markdown` pills; additive (0.10.5); approved by the owner, built after the Scrollbar. |
| [PopupMenu toggle items and columns](docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md) | 2026-10-28 | Implemented | `PopupItemKind::Toggle` (`PopupItem::toggle`) flips its `checked` in place, keeps the menu open and the highlight, and queues `Toggled { id, checked }` instead of `Chosen`; rows may carry cells (`PopupRow` of an item and `PopupCell`s: text, `TextAlign`, `Auto`/`Fixed`/`Flex` width, dim, a size scale) drawn in columns shared across the menu, cut with …, centred on the row, opened with `open_rows` so `PopupItem` is unchanged; `column_gap` and `dim_text_color` in the theme; additive (0.10.6, after MarkdownView's 0.10.5); asked by flankr's bottom bar and plugin plans and approved by the owner, being built as 0.10.6 on 0.10.5. |
