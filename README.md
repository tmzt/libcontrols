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
| [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md) | 2026-10-16 | Draft | Fix the host's routing contract for 0.3.0: an app-owned `InputFilter` stack asked first (`Pass`, `Consumed`, `Redirect`), area routing with a grab, focus routing with `FocusEvent`s, hover as enter and leave, modifiers on every event in an `EventContext` (no `set_modifiers`), `Symbol` names and `Key::Back`; no per-control work per event; draft awaiting the owner's review. |
| [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md) | 2026-10-16 | Draft | Build modals on plan 02's `Redirect`: a persistent `ModalState` stack of four, a `ModalFilter` registered once that traps input to a hosted `ModalStack`, app-drawn frames, hold-open veto, Escape and Back dismissal; cuts 0.3.0 in its last step; draft awaiting the owner's review. |
