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
