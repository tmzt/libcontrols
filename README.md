# libcontrols

Immediate-mode UI controls drawn as MSDF primitives.

`libcontrols` provides a set of reusable UI controls (labels, buttons, sliders, radios, option grids, panels) that render directly into [`libmsdf`](https://github.com/tmzt/libmsdf) draw lists. Controls are stateless and pure—they contain no internal event loops or state machines.

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

## Labels and tooltips

A label written as `"XX - Name"` draws the two-letter abbreviation in its cell
and shows the name as a tooltip on hover. `Radio` and `Options` take
`.with_abbreviate(false)` to draw the full name instead, with no tooltip, when
the row has room for whole words. A label without the `" - "` separator is drawn
as written.

## Usage Pattern

1. **Build controls** with a fluent builder API:
   ```rust
   let button = Button::new("Click me", Rect::new(10.0, 10.0, 100.0, 30.0))
       .with_active(true);
   ```

2. **Register with ControlHost** for event routing:
   ```rust
   let mut host = ControlHost::builder()
       .with_named_control("my_button", button.handle())
       .build();
   ```

3. **Deliver input events**:
   ```rust
   let response = host.deliver_mouse(event);
   let response = host.deliver_keyboard(event);
   ```

4. **Draw to a DrawList**:
   ```rust
   button.draw(&mut draw_list, &draw_context);
   ```

Read control values through their handles:
```rust
let handle = button.handle();
let text: String = handle.read();
let is_active: bool = handle.read();
```

## Input routing

Each event a `ControlHost` is given goes first to its filters (below), then:

- **Pointer events and gestures** go to the control whose area contains the
  pointer, searching from the last control added. A press grabs that control:
  until the button is released, moves, drags and the release go to it even off
  its area. A plain move outside a grab only updates hover.
- **Keys** go to the focused control. Tab and BackTab move the focus and Escape
  clears it; those three reach no control. `Key::Back` is an ordinary key.
- **Focus changes** arrive as `handle_focus(FocusEvent::Gained)` and `Lost`.
- **Hover** is enter and leave: only the control the pointer left and the one
  it entered are told.
- **Modifiers** arrive with every event in its `EventContext`
  (`ctx.modifiers`, and `ctx.press_modifiers` for the press that started a
  drag). No control is told when Shift goes down.

Controls are named with interned `Symbol`s. `sym!("speed")` interns once per
call site; a `&str` name still works anywhere a name is taken, and
`response.target_id == Some(sym!("speed"))` compares two integers.

A host rebuilt per event starts empty. Carry what should survive into the next
build: `focused_key()` / `set_focused_key()`, `active_drag()` /
`set_active_drag()`, `hovered_key()` / `set_hovered_key()`, and
`modifiers()` / `set_modifiers()`.

A filter is an app-owned object that sees every event first, including Tab,
Escape and modifier changes, and may pass it on, consume it, or redirect it to
one named control. Register one only when there is a reason:

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
let host = ControlHost::builder().with_filter(&SaveShortcut).build();
```

## Modals

A `ModalState` is the app's stack of open modals (up to four), kept in an
`Arc` across host builds. While it holds any modal, a `ModalFilter` (created
once from the state) redirects every event to a `ModalStack` control, so
nothing beneath receives input: the base UI is level 0. The stack hands each
event to the top modal and draws every modal, bottom to top, above the rest of
the host. Escape, the platform back key and a tap outside the frame dismiss the
top modal; `with_hold_open(true)` refuses those and reports
`ModalEvent::DismissRefused` instead, for unsaved changes. Transitions are
`Timed` (moved by `advance`), `Driven` (the app sets progress) or `Instant`,
and the look of each modal is a `ModalFrame` the app can implement.

```rust
// Once, in app state:
let modals = Arc::new(ModalState::new());
let modal_filter = modals.filter(sym!("modals"));

// The settings icon pushes a modal; its Quit button pushes a confirm over it.
modals.push(sym!("settings"));
modals.push(sym!("confirm"));

// Every frame:
modals.advance(dt);
for event in modals.take_events() { /* Closed { id, by }, DismissRefused { .. } */ }

// Every host build: one Modal per stack entry, the stack added last.
let mut stack = ModalStack::new(modals.clone(), viewport);
for entry in modals.stack() {
    stack = stack.with_modal(if entry.id == sym!("settings") {
        Modal::centered(modals.clone(), entry.id, viewport, [400.0, 300.0], 16.0)
            .with_hold_open(unsaved_changes)
            .with_named_control(sym!("quit"), quit.handle())
    } else {
        Modal::centered(modals.clone(), entry.id, viewport, [240.0, 120.0], 12.0)
            .with_named_control(sym!("cancel"), cancel.handle())
            .with_close_control(sym!("cancel"))
    });
}
let host = ControlHost::builder()
    .with_named_control(sym!("modals"), stack.handle())
    .with_filter(&modal_filter)
    .build();
```

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
| [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md) | 2026-10-16 | In implementation | Fix the host's routing contract for 0.3.0: an app-owned `InputFilter` stack asked first (`Pass`, `Consumed`, `Redirect`), area routing with a grab, focus routing with `FocusEvent`s, hover as enter and leave, modifiers on every event in an `EventContext` (no `set_modifiers`), `Symbol` names and `Key::Back`; no per-control work per event; approved, being implemented on `impl/0.3.0`. |
| [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md) | 2026-10-16 | In implementation | Build modals on plan 02's `Redirect`: a persistent `ModalState` stack of four, a `ModalFilter` registered once that traps input to a hosted `ModalStack`, app-drawn frames, hold-open veto, Escape and Back dismissal; cuts 0.3.0 in its last step; approved, being implemented on `impl/0.3.0`. |
