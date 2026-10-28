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
- **ScrollPane** — A viewport onto children laid out on a larger plane, scrolled by touch, wheel, trackpad and keys

## Labels and tooltips

A label written as `"XX - Name"` draws the two-letter abbreviation in its cell
and shows the name as a tooltip on hover. `Radio` and `Options` take
`.with_abbreviate(false)` to draw the full name instead, with no tooltip, when
the row has room for whole words. A label without the `" - "` separator is drawn
as written.

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

## Building

```bash
cargo build
cargo test
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
| [Drawer, composited from offscreen layers](docs/design/20261028_01_DRAWER_PLAN.md) | 2026-10-28 | Draft | A `Drawer` (left, right, top or bottom) with a content router, opened and closed by API, edge swipe, drag, fling, scrim tap and Back; while visible, main screen and sheet are kept in wgpu textures redrawn only on change and each frame is one composite driven by an eased uniform `t`, built on wgpu in libcontrols; at `t = 0` no texture, pass or copy, the main view draws straight to the surface; additive (0.9.2); Draft, awaiting the owner. |
