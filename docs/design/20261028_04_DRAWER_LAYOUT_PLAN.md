---
title: DrawerLayout, the parent of a main view and a drawer
date: 2026-10-28
status: In implementation
covers: >
  A small container, `DrawerLayout`, with exactly two children: the main
  view (a `Router`) and the drawer's content (the `Router` inside its
  `Drawer`). It owns their layout (the main view fills the layout's rect;
  the sheet is placed by the drawer's edge and size, offscreen when
  closed), holds the `Drawer` and its shared `DrawerState`, routes input
  (to the main view normally; to the sheet's content while the sheet is
  visible; points on the scrim reported as `Scrim`), forwards the drawer's
  calls, and is the place the drawer's compositing hooks in (it draws the
  main view and the sheet separately for `DrawerLayers`, or both directly
  into one list). Steps and tests.
sources:
  - >
    The owner (2026-10-28), verbatim: "approve impl in subagent, test drawer
    behavior, this might also need a small drawerlayout parent compinent
    with main and drawer as children so plan and impl that first".
  - >
    docs/design/20261028_01_DRAWER_PLAN.md, its 12:32 EDT amendment: "A
    `DrawerLayout` parent comes first, in its own plan
    (`docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md`)".
  - >
    libcontrols at 30fb943 (0.9.1): src/router.rs (`Router::render` 583,
    `draw_subtree`'s `shift` 633-660, which turns a screen point into a
    node's coordinates and places a scroll pane's clip, 668-672;
    `deliver_event` 778; `cancel_grab` 1079 and `clear_hover` 747;
    `advance` 1603); src/modal.rs (`ModalDialog` 1018, the existing node
    that owns a content `Router`, which it lays out in absolute
    coordinates and forwards events to unchanged, 1103-1235).
related:
  - docs/design/20261028_01_DRAWER_PLAN.md (the drawer this layout holds;
    built on this plan, its steps adapted to it)
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section 4
    the router and section 8.5 the theming standard this layout's drawer
    follows)
---

# DrawerLayout, the parent of a main view and a drawer

## 1. Goal

The drawer plan (plan 01) makes a `Drawer` that sits in front of whatever
the app has. The owner asked for a small parent that holds the two sides
as children, so an app gets a working drawer screen by filling two
routers and calling one `deliver`, one `advance` and one draw.

After this plan:

- `DrawerLayout::new(rect, edge, options)` makes a layout with an empty
  main `Router`, filling `rect`, and a `Drawer` on `edge` whose content
  `Router` is laid out in the sheet's own coordinates;
- the app adds controls to `layout.main_mut()` and `layout.content_mut()`
  as to any router;
- `layout.deliver(event)` routes every event and says where it went;
- `layout.open()`, `close()`, `toggle()`, `set_open_now()`,
  `set_progress()` and `settle()` drive the drawer;
- `layout.render(list, ctx)` draws both sides into one list (the direct
  path), and `render_main` / `render_sheet` draw each alone for the
  drawer's textures (plan 01's `DrawerLayers`).

## 2. Decisions taken from the code

- **Both children are `Router`s.** A router is the crate's subtree: it
  owns nodes, focus, hover, grab and its scroll panes, and a single custom
  control is a router with one node. `ModalDialog` already owns a content
  `Router` the same way. A main view that draws and routes itself (the
  companion's overview) goes in as one node of the main router.
- **The main router is in the layout's coordinates** (absolute, as every
  root router is). **The content router is in sheet coordinates**, its
  origin the sheet's top-left (plan 01, section 3.1), so its texture is
  sheet-sized and its content does not move while the sheet does. Events
  for it are moved by `-sheet(t).pos()`.
- **The layout is not a router node.** It is the root of a screen; nesting
  it inside another router would need its two-pass compositing inside a
  single `DrawControl::draw`, which the textures cannot do. A nested use
  is out of scope.
- **Drawing the content at an offset.** In the direct path the content is
  drawn under a `push_translate` of the sheet's position. A scroll pane in
  the content places its clip in screen pixels from the router's `shift`
  (router.rs:668-672), which starts at zero for a root, so a translated
  root would clip its panes in the wrong place. A crate-internal
  `Router::render_at(list, ctx, origin)` starts `shift` at `-origin`; it
  is `render` when `origin` is zero.

## 3. Design

### 3.1 Types

```rust
/// A main view and a drawer over it (src/drawer/layout.rs).
pub struct DrawerLayout {
    rect: Cell<Rect>,
    main: Router,
    drawer: Drawer,
}

impl DrawerLayout {
    pub fn new(rect: Rect, edge: DrawerEdge, options: DrawerOptions) -> Self;
    /// Share a `DrawerState` (it survives the layout being rebuilt).
    pub fn with_state(self, state: Arc<DrawerState>) -> Self;
    /// The node of the content focused when the drawer reaches Open.
    pub fn with_initial_focus(self, id: NodeId) -> Self;

    pub fn rect(&self) -> Rect;
    /// Moves the main view's rect and the drawer's viewport together.
    pub fn set_rect(&self, rect: Rect);

    pub fn main(&self) -> &Router;
    pub fn main_mut(&mut self) -> &mut Router;
    pub fn drawer(&self) -> &Drawer;
    pub fn drawer_mut(&mut self) -> &mut Drawer;
    /// The drawer's content router (sheet coordinates).
    pub fn content(&self) -> &Router;
    pub fn content_mut(&mut self) -> &mut Router;
    pub fn state(&self) -> &Arc<DrawerState>;

    // The drawer's calls, forwarded to its state (plan 01, 11:22 EDT).
    pub fn open(&self);
    pub fn close(&self);
    pub fn toggle(&self);
    pub fn set_open_now(&self, open: bool);
    pub fn set_progress(&self, t: f32);
    pub fn settle(&self, open: bool, velocity: f32);

    /// Route one event (section 3.2).
    pub fn deliver(&self, event: InputEvent) -> DrawerRoute;
    /// The drawer's animation, the content's and the main view's own
    /// (flings, indicators). Whether anything still moves.
    pub fn advance(&self, dt_secs: f32) -> bool;
    /// Resolve the drawer's options against `theme` now (it is also done at
    /// every draw with the context's theme).
    pub fn set_theme(&self, theme: &ControlTheme);

    /// The direct path: the main view, then the drawer's scrim, shadow and
    /// sheet over it (plan 01, section 3.10) when it is visible.
    pub fn render(&self, list: &mut DrawList, ctx: &DrawContext);
    /// The main view alone: to the surface while `LayerPlan::direct`, or
    /// into `DrawerLayers::main_view()`.
    pub fn render_main(&self, list: &mut DrawList, ctx: &DrawContext);
    /// The sheet alone, in sheet coordinates, for its texture.
    pub fn render_sheet(&self, list: &mut DrawList, ctx: &DrawContext);
    /// Bump the main view's revision (the app changed what it draws).
    pub fn mark_main(&self);
}
```

`DrawerRoute` is plan 01's, with the main view's response carried:

```rust
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DrawerRoute {
    /// The main view's (from `DrawerLayout::deliver`, its response;
    /// from `Drawer::deliver`, `None`: give it to the main view).
    Main(Option<EventResponse>),
    /// The sheet's content took it; its response.
    Drawer(Option<EventResponse>),
    /// A point on the scrim while the drawer is visible. Nothing took it;
    /// the app may call `close()`.
    Scrim,
}
```

### 3.2 Routing

`deliver` asks the drawer first (`Drawer::deliver`, plan 01's content
routing), then gives the event to the main view when the drawer answers
`Main(None)`:

- **The drawer is visible** when its phase is not `Closed` and `t > 0`.
- **A press belongs to where its `Down` landed** until the next `Down`:
  its `Drag`, `Up` and `Click` go to the same side wherever they are, so a
  press on the main view that opens the drawer (a menu button) still ends
  on the main view, and a drag started in the sheet stays with the
  content. `Down`: visible and inside `sheet(t)`, the content; visible,
  inside the layout's rect and outside the sheet, the scrim; otherwise the
  main view.
- **Unpressed pointer events** (`Move`, `Scroll`, `ScrollPixels`) go by
  position by the same rule. A pointer event that goes to the content or
  the scrim clears the main view's hover; one that leaves the sheet clears
  the content's.
- **Keys and gestures** go to the content while the phase is `Opening`,
  `Open` or `Dragging` with `t > 0`; otherwise to the main view.
- **Modifiers** go to both.

### 3.3 Layout

- `main` is laid out by the app inside `rect` (absolute). `set_rect` sets
  the drawer's viewport to the same rect (plan 01's `set_viewport`, which
  bumps the content mark).
- The sheet is `Drawer::sheet()`: placed by edge and extent at `t`, fully
  outside `rect` at `t = 0` (offscreen). Its content router is laid out by
  the app from `(0, 0)` at `Drawer::sheet_open().size()`.

### 3.4 Where compositing hooks in

The drawer keeps two revisions (`PaneMark`): `content_mark` (the sheet)
and `main_mark` (the main view). The layout bumps `main_mark` on every
event it gives the main view and on every `advance` in which the main view
still moves; the app bumps it with `mark_main()` for changes the layout
cannot see. `DrawerLayers::prepare(device, layout.drawer(), surface,
scale)` reads both. A frame:

```rust
let plan = layers.prepare(device, layout.drawer(), size, scale);
if plan.direct {
    list.clear(); layout.render_main(&mut list, &ctx);  // to the surface, as today
} else {
    if plan.draw_main  { list.clear(); layout.render_main(&mut list, &ctx);  /* into layers.main_view() */ }
    if plan.draw_sheet { sheet.clear(); layout.render_sheet(&mut sheet, &ctx);
                         layers.render_sheet(device, queue, &renderer, &sheet, set); }
    layers.composite(device, queue, target, layout.drawer(), scale);
}
```

## 4. Out of scope

Nesting a layout inside a router; more than one drawer per layout; any
gesture (plan 01's 11:22 EDT amendment: the app drives the drawer by
calls).

## 5. Tests

- **Layout**: the main router is untouched by the drawer; `sheet()` is
  outside `rect` when closed, inside when open, for each edge;
  `set_rect` moves the drawer's viewport and keeps `t`.
- **Routing**: closed, a tap on a main button activates it
  (`Main(Some(..))`); open, a tap on a content button activates it with
  the position moved into sheet coordinates (`Drawer(Some(..))`), and the
  main button under the same point does not; a tap on the scrim is
  `Scrim` and reaches nothing; a press that began on the main view and
  ends after `open()` ends on the main view; keys go to the content while
  open and to the main view while closed or closing.
- **Calls**: each forwarded call changes the shared state as the same
  call on `DrawerState` does.
- **Drawing**: `render` while closed equals `render_main`; while open it
  adds the scrim, the sheet's background and the content's instances
  inside one clip at `sheet(t)`; `render_at` places a scroll pane's clip
  at the sheet's offset.
- **Marks**: an event given to the main view bumps `main_mark`; one given
  to the content bumps `content_mark`.

## 6. Steps

1. **`Router::render_at`** (crate-internal).
   **Done when:** a scroll pane in a router drawn at an origin has its
   clip moved by that origin, and every existing test passes.
2. **`DrawerLayout`** with its accessors, the forwarded calls, `set_rect`
   and the three draws, on plan 01's `Drawer` (plan 01 step 3).
   **Done when:** the layout and drawing tests pass.
3. **Routing**: `deliver` (section 3.2) on plan 01's `Drawer::deliver`, and
   the marks (section 3.4).
   **Done when:** the routing and mark tests pass.
4. **The compositing hook**: plan 01's `DrawerLayers` driven from a
   layout, as section 3.4 shows, in plan 01's GPU test.
   **Done when:** plan 01's step 8 passes with the layout as its drawer's
   parent.

## Amendment — 2026-10-28 12:37 EDT: approved; implementation started

The owner's words in this plan's sources ("plan and impl that first")
approve it and ask for it to be built before the drawer. Implementation
has started and this document is now immutable; it changes only on the
owner's explicit instruction.
