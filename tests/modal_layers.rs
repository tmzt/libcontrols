//! The modal stack's layers: any control as a modal, layer filters, shapes,
//! RAII popping, and the order of the input chains.

mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::Recorder;
use libcontrols::filter::{FilterContext, FilterStack, Filtered, InputFilter};
use libcontrols::{
    Button, ControlAction, ControlPart, DismissOn, DismissReason, DrawContext, DrawControl,
    EventContext, FocusEvent, HostedControl, InputEvent, Key, KeyboardEvent, LayerContext,
    LayerFilter, LayerMask, LayerView, Modal, ModalDialog, ModalError, ModalEvent, ModalOptions,
    ModalStack, ModalState, ModalTransition, MouseButton, MouseEvent, NodeFlags, Rect, Router,
    ShapedLayerFilter,
};
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const VIEWPORT: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);

/// A self-drawn modal: a circle (or box) it draws itself, which logs what it
/// is given and can ask to close itself on Enter.
struct Blob {
    rect: Rect,
    circle: bool,
    shaped: bool,
    seen: RefCell<Vec<InputEvent>>,
    layers: RefCell<Vec<Option<LayerView>>>,
    dismiss_on_enter: bool,
    options: ModalOptions,
    filter_drops: Option<Rc<()>>,
}

impl Blob {
    fn new(rect: Rect) -> Self {
        Self {
            rect,
            circle: false,
            shaped: false,
            seen: RefCell::new(Vec::new()),
            layers: RefCell::new(Vec::new()),
            dismiss_on_enter: false,
            options: ModalOptions::default(),
            filter_drops: None,
        }
    }
}

impl DrawControl for Blob {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.layers.borrow_mut().push(ctx.layer);
        let progress = ctx.layer.map_or(1.0, |l| l.progress);
        // The shape grows with progress, so a drawn mask must follow it.
        let size = [self.rect.w * progress, self.rect.h * progress];
        list.push(SdfInstance {
            kind: if self.circle {
                SdfKind::Circle
            } else {
                SdfKind::Box
            },
            position: self.rect.pos(),
            size,
            color: [0.2, 0.4, 0.6, 1.0],
            anim: 0,
        });
    }
}

impl HostedControl for Blob {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect.contains(pos).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.seen.borrow_mut().push(InputEvent::Mouse(*event));
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        self.seen.borrow_mut().push(InputEvent::Keyboard(*event));
        (self.dismiss_on_enter && event.key() == Some(Key::Enter))
            .then_some(ControlAction::Dismiss { control: None })
    }
}

/// A layer filter that only marks its own drop, around the default one.
struct Tracked {
    inner: Box<dyn LayerFilter>,
    _alive: Rc<()>,
}

impl InputFilter for Tracked {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        self.inner.filter(event, ctx)
    }
}

impl LayerFilter for Tracked {}

impl Modal for Blob {
    fn options(&self) -> ModalOptions {
        self.options
    }

    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        let inner: Box<dyn LayerFilter> = if self.shaped {
            Box::new(ShapedLayerFilter::new(LayerMask::drawn(0.0), layer.clone()))
        } else {
            Box::new(libcontrols::ModalLayerFilter::new(
                self.bounds(),
                layer.clone(),
            ))
        };
        match &self.filter_drops {
            Some(alive) => Box::new(Tracked {
                inner,
                _alive: Rc::clone(alive),
            }),
            None => inner,
        }
    }
}

fn setup() -> (Router, Arc<ModalState>, libcontrols::NodeId) {
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let mut router = Router::new();
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    (router, state, stack)
}

/// What the Blob layer `id` has seen so far, emptying its log.
fn seen(router: &Router, stack: libcontrols::NodeId, id: &str) -> Vec<InputEvent> {
    let stack = router.get::<ModalStack>(stack).unwrap();
    let b = stack.modal::<Blob>(id).expect("a Blob layer");
    std::mem::take(&mut *b.seen.borrow_mut())
}

fn down(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Down {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn up(x: f32, y: f32) -> MouseEvent {
    MouseEvent::Up {
        pos: [x, y],
        button: MouseButton::Left,
    }
}

fn key(k: Key) -> KeyboardEvent {
    KeyboardEvent::KeyDown { key: k }
}

fn render(router: &Router) -> DrawList {
    let mut list = DrawList::new();
    router.render(&mut list, &DrawContext::default());
    list
}

fn dismissed(state: &ModalState) -> Vec<DismissReason> {
    state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::Dismissed { by, .. } => Some(by),
            _ => None,
        })
        .collect()
}

#[test]
fn installing_names_the_stack() {
    let (router, _state, stack) = setup();
    assert_eq!(router.find(ModalStack::NAME), Some(stack));
    assert_eq!(router.flags(stack), NodeFlags::NONE, "never a Tab stop");
}

#[test]
fn a_second_install_in_the_same_router_is_refused() {
    let (mut router, state, _stack) = setup();
    assert_eq!(
        ModalStack::install(&mut router, state, VIEWPORT),
        Err(ModalError::AlreadyInstalled)
    );
}

#[test]
fn pushing_without_a_stack_is_an_error() {
    let mut router = Router::new();
    assert_eq!(
        router.push_modal("m", Blob::new(Rect::new(0.0, 0.0, 10.0, 10.0))),
        Err(ModalError::NoStack)
    );
}

#[test]
fn the_stack_stays_after_roots_added_later() {
    let (mut router, _state, stack) = setup();
    let late = router.add(None, Recorder::at(0.0));
    let roots: Vec<_> = router.children(None).collect();
    assert_eq!(roots, [late, stack], "the library root stays last");
}

#[test]
fn more_than_four_modals_can_be_pushed() {
    let (mut router, state, _stack) = setup();
    for i in 0..6 {
        router
            .push_modal(format!("m{i}"), Blob::new(Rect::new(0.0, 0.0, 10.0, 10.0)))
            .unwrap();
    }
    assert_eq!(state.depth(), 6);
    assert_eq!(
        router.push_modal("m3", Blob::new(Rect::new(0.0, 0.0, 10.0, 10.0))),
        Err(ModalError::AlreadyOpen)
    );
}

#[test]
fn nothing_reaches_the_app_while_a_layer_is_up() {
    let (mut router, state, stack) = setup();
    let app = router.add(None, Recorder::at(0.0));
    router
        .push_modal("m", Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    for event in [
        InputEvent::Mouse(down(10.0, 10.0)),
        InputEvent::Keyboard(key(Key::Enter)),
    ] {
        router.deliver_event(event);
    }
    assert!(router.get::<Recorder>(app).unwrap().take().is_empty());
    let resp = router.deliver_mouse(down(310.0, 310.0));
    assert_eq!(resp.target, Some(stack), "the response names the stack");
    assert_eq!(
        seen(&router, stack, "m").len(),
        2,
        "the Enter and the press inside"
    );
    state.pop();
    let resp = router.deliver_mouse(down(10.0, 10.0));
    assert_eq!(
        resp.target,
        Some(app),
        "routing resumes once the layer is gone"
    );
}

#[test]
fn an_outside_click_dismisses_or_is_only_blocked_by_option() {
    for dismiss in [true, false] {
        let (mut router, state, _stack) = setup();
        let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
        b.options = ModalOptions::default().with_dismiss(DismissOn {
            outside_tap: dismiss,
            ..DismissOn::default()
        });
        router.push_modal("m", b).unwrap();
        state.take_events();
        router.deliver_mouse(down(10.0, 10.0));
        router.deliver_mouse(up(10.0, 10.0));
        router.deliver_mouse(MouseEvent::Click {
            pos: [10.0, 10.0],
            button: MouseButton::Left,
        });
        let expected: &[DismissReason] = if dismiss {
            &[DismissReason::OutsideTap]
        } else {
            &[]
        };
        assert_eq!(dismissed(&state), expected, "outside_tap = {dismiss}");
        if !dismiss {
            assert!(state.contains("m"), "only blocked");
        }
    }
}

#[test]
fn a_drag_that_began_inside_reaches_the_modal_outside_its_shape() {
    let (mut router, state, stack) = setup();
    router
        .push_modal("m", Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    state.take_events();
    router.deliver_mouse(down(310.0, 310.0));
    router.deliver_mouse(MouseEvent::Drag {
        start_pos: [310.0, 310.0],
        current_pos: [10.0, 10.0],
        delta: [-300.0, -300.0],
        button: MouseButton::Left,
    });
    router.deliver_mouse(up(10.0, 10.0));
    let kinds: Vec<_> = seen(&router, stack, "m")
        .into_iter()
        .map(|e| match e {
            InputEvent::Mouse(MouseEvent::Down { .. }) => "down",
            InputEvent::Mouse(MouseEvent::Drag { .. }) => "drag",
            InputEvent::Mouse(MouseEvent::Up { .. }) => "up",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["down", "drag", "up"]);
    assert!(dismissed(&state).is_empty(), "its release cannot dismiss");
}

#[test]
fn escape_and_back_dismiss_and_can_be_turned_off() {
    let (mut router, state, stack) = setup();
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.options = ModalOptions::default().with_dismiss(DismissOn {
        escape: false,
        ..DismissOn::default()
    });
    router.push_modal("m", b).unwrap();
    state.take_events();
    router.deliver_keyboard(key(Key::Escape));
    assert!(dismissed(&state).is_empty());
    assert_eq!(
        seen(&router, stack, "m").len(),
        1,
        "Escape reached the modal"
    );
    router.deliver_keyboard(key(Key::Back));
    assert_eq!(dismissed(&state), [DismissReason::Back]);
}

#[test]
fn hold_open_refuses_every_user_dismissal() {
    let (mut router, state, _stack) = setup();
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.dismiss_on_enter = true;
    b.options = ModalOptions::default().with_hold_open(true);
    router.push_modal("m", b).unwrap();
    state.take_events();
    router.deliver_keyboard(key(Key::Escape));
    router.deliver_keyboard(key(Key::Back));
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(up(10.0, 10.0));
    router.deliver_keyboard(key(Key::Enter));
    let refused: Vec<DismissReason> = state
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            ModalEvent::DismissRefused { by, .. } => Some(by),
            _ => None,
        })
        .collect();
    assert_eq!(
        refused,
        [
            DismissReason::Escape,
            DismissReason::Back,
            DismissReason::OutsideTap,
            DismissReason::Requested
        ]
    );
    assert!(state.contains("m"));
    state.pop();
    assert!(
        !state.contains("m"),
        "the app's own dismissal is never held"
    );
}

#[test]
fn a_self_drawn_modal_dismisses_itself() {
    let (mut router, state, _stack) = setup();
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.dismiss_on_enter = true;
    router.push_modal("m", b).unwrap();
    state.take_events();
    router.deliver_keyboard(key(Key::Enter));
    assert_eq!(dismissed(&state), [DismissReason::Requested]);
}

#[test]
fn only_the_top_layers_filter_is_asked() {
    let (mut router, state, stack) = setup();
    router
        .push_modal("lower", Blob::new(Rect::new(0.0, 0.0, 800.0, 600.0)))
        .unwrap();
    router
        .push_modal("upper", Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    state.take_events();
    router.deliver_mouse(down(10.0, 10.0));
    router.deliver_mouse(up(10.0, 10.0));
    assert_eq!(
        dismissed(&state),
        [DismissReason::OutsideTap],
        "the upper's filter"
    );
    assert!(
        seen(&router, stack, "lower").is_empty(),
        "the lower layer saw nothing"
    );
    assert!(state.contains("lower"));
}

struct Accelerator(Rc<Cell<u32>>);

impl InputFilter for Accelerator {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        match event {
            InputEvent::Keyboard(KeyboardEvent::KeyDown {
                key: Key::Char('q'),
            }) if ctx.modifiers.ctrl => {
                self.0.set(self.0.get() + 1);
                Filtered::Consumed(Some(ControlAction::Custom("quit".into())))
            }
            _ => Filtered::Pass,
        }
    }
}

#[test]
fn a_global_accelerator_works_while_a_modal_is_open() {
    let filters = FilterStack::new();
    let hits = Rc::new(Cell::new(0));
    filters.add(Accelerator(Rc::clone(&hits)));
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Instant));
    let mut router = Router::new().with_filter_stack(filters);
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    router
        .push_modal("m", Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0)))
        .unwrap();
    router.deliver_modifiers(libcontrols::Modifiers {
        ctrl: true,
        ..Default::default()
    });
    let resp = router.deliver_keyboard(key(Key::Char('q')));
    assert_eq!(hits.get(), 1, "the global chain runs first");
    assert_eq!(resp.action, Some(ControlAction::Custom("quit".into())));
    assert!(
        seen(&router, stack, "m").is_empty(),
        "the modal never saw it"
    );
}

#[test]
fn popping_drops_the_layer_and_its_filter_leaves_the_stack() {
    let (mut router, state, stack) = setup();
    let alive = Rc::new(());
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.filter_drops = Some(Rc::clone(&alive));
    router.push_modal("m", b).unwrap();
    assert_eq!(
        Rc::strong_count(&alive),
        3,
        "ours, the modal's and its filter's"
    );
    state.pop();
    render(&router);
    assert_eq!(
        Rc::strong_count(&alive),
        1,
        "popping dropped the modal and its filter"
    );
    assert_eq!(router.get::<ModalStack>(stack).unwrap().len(), 0);
}

#[test]
fn a_closed_modals_content_is_dropped() {
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Timed {
        open_secs: 0.1,
        close_secs: 0.1,
    }));
    let mut router = Router::new();
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    let mut dialog = ModalDialog::centered(VIEWPORT, [200.0, 100.0], 10.0);
    dialog
        .content_mut()
        .add(None, Button::new("OK", Rect::new(310.0, 260.0, 60.0, 20.0)));
    router.push_modal("d", dialog).unwrap();
    state.advance(1.0);
    state.pop();
    render(&router);
    let s = router.get::<ModalStack>(stack).unwrap();
    assert_eq!(s.len(), 1, "still drawn while closing");
    state.advance(1.0);
    render(&router);
    let s = router.get::<ModalStack>(stack).unwrap();
    assert_eq!(s.len(), 0, "gone once closed");
    assert!(s.modal::<ModalDialog>("d").is_none());
}

#[test]
fn a_modal_is_built_once_and_keeps_its_focus_across_frames() {
    common::take_focus_journal();
    let (mut router, _state, stack) = setup();
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let a = dialog.content_mut().add(
        None,
        Recorder {
            rect: Rect::new(270.0, 220.0, 50.0, 50.0),
            ..Recorder::at(0.0)
        },
    );
    let b = dialog.content_mut().add(
        None,
        Recorder {
            rect: Rect::new(330.0, 220.0, 50.0, 50.0),
            ..Recorder::at(0.0)
        },
    );
    router
        .push_modal("d", dialog.with_initial_focus(a))
        .unwrap();
    router.deliver_keyboard(key(Key::Tab));
    for _ in 0..5 {
        render(&router);
        router.deliver_mouse(MouseEvent::Move { pos: [5.0, 5.0] });
    }
    let s = router.get::<ModalStack>(stack).unwrap();
    let d = s.modal::<ModalDialog>("d").unwrap();
    assert_eq!(
        d.content().focused(),
        Some(b),
        "the Tab moved it from a to b and it stays"
    );
    let journal = common::take_focus_journal();
    let gained = journal
        .iter()
        .filter(|(_, f)| *f == FocusEvent::Gained)
        .count();
    assert_eq!(
        gained, 2,
        "a once (initial), b once (Tab): no repeat across frames"
    );
}

#[test]
fn no_gained_is_repeated_while_a_modal_stays_open() {
    common::take_focus_journal();
    let (mut router, _state, _stack) = setup();
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let a = dialog.content_mut().add(
        None,
        Recorder {
            rect: Rect::new(270.0, 220.0, 50.0, 50.0),
            ..Recorder::at(0.0)
        },
    );
    router
        .push_modal("d", dialog.with_initial_focus(a))
        .unwrap();
    for _ in 0..10 {
        router.deliver_keyboard(key(Key::Enter));
        render(&router);
    }
    let journal = common::take_focus_journal();
    assert_eq!(journal, [(270.0, FocusEvent::Gained)]);
}

#[test]
fn a_close_control_dismisses_with_its_node() {
    let (mut router, state, _stack) = setup();
    let mut dialog = ModalDialog::centered(VIEWPORT, [300.0, 200.0], 10.0);
    let cancel = dialog.content_mut().add(
        None,
        Button::new("Cancel", Rect::new(270.0, 220.0, 80.0, 30.0)),
    );
    router
        .push_modal("d", dialog.with_close_control(cancel))
        .unwrap();
    state.take_events();
    router.deliver_mouse(down(280.0, 230.0));
    assert_eq!(dismissed(&state), [DismissReason::Control(cancel)]);
}

#[test]
fn a_shaped_modal_uses_its_own_filter_layer() {
    let (mut router, state, _stack) = setup();
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.circle = true;
    b.shaped = true;
    router.push_modal("m", b).unwrap();
    state.take_events();
    render(&router);
    // A corner of the box is outside the circle the modal drew.
    router.deliver_mouse(down(302.0, 302.0));
    router.deliver_mouse(up(302.0, 302.0));
    assert_eq!(
        dismissed(&state),
        [DismissReason::OutsideTap],
        "the corner is outside"
    );
}

#[test]
fn a_drawn_mask_follows_the_layers_sdf_as_it_animates() {
    let state = Arc::new(ModalState::new().with_transition(ModalTransition::Driven));
    let mut router = Router::new();
    let stack = ModalStack::install(&mut router, Arc::clone(&state), VIEWPORT).unwrap();
    let mut b = Blob::new(Rect::new(300.0, 300.0, 100.0, 100.0));
    b.shaped = true;
    b.options = ModalOptions::default().with_dismiss(DismissOn {
        outside_tap: false,
        ..DismissOn::default()
    });
    router.push_modal("m", b).unwrap();
    state.set_progress("m", 1.0);
    state.set_progress("m", 0.5);
    // Half grown: the box covers (300..350, 300..350).
    render(&router);
    router.deliver_mouse(MouseEvent::Click {
        pos: [380.0, 380.0],
        button: MouseButton::Left,
    });
    assert!(
        seen(&router, stack, "m").is_empty(),
        "outside the half-grown shape"
    );
    state.set_progress("m", 1.0);
    render(&router);
    router.deliver_mouse(MouseEvent::Click {
        pos: [380.0, 380.0],
        button: MouseButton::Left,
    });
    assert_eq!(
        seen(&router, stack, "m").len(),
        1,
        "inside once fully grown"
    );
}

#[test]
fn a_texture_mask_decides_inside_by_alpha() {
    // A 2x2 mask over (300..400, 300..400): only the top-left cell is opaque.
    let mask = LayerMask::texture(
        Rect::new(300.0, 300.0, 100.0, 100.0),
        2,
        2,
        Arc::from(vec![255u8, 0, 0, 0].into_boxed_slice()),
        128,
    );
    use libcontrols::LayerShape;
    assert!(mask.contains([310.0, 310.0]), "the opaque cell");
    assert!(!mask.contains([390.0, 310.0]), "a clear cell");
    assert!(!mask.contains([390.0, 390.0]));
    assert!(!mask.contains([10.0, 10.0]), "outside its rect");
    assert!(mask.contains([300.0, 300.0]), "its top-left corner");
}

#[test]
fn a_drawn_mask_ignores_text_and_textures() {
    use libcontrols::LayerShape;
    let mask = LayerMask::drawn(0.0);
    let mut list = DrawList::new();
    list.push(SdfInstance {
        kind: SdfKind::Box,
        position: [300.0, 300.0],
        size: [20.0, 20.0],
        color: [1.0; 4],
        anim: 0,
    });
    list.push(SdfInstance {
        kind: SdfKind::MsdfText {
            char_start: 0,
            char_count: 0,
            px_range: 4.0,
        },
        position: [500.0, 300.0],
        size: [100.0, 100.0],
        color: [1.0; 4],
        anim: 0,
    });
    mask.drawn(&list, 0..list.instances.len());
    assert!(mask.contains([305.0, 305.0]), "the box counts");
    assert!(!mask.contains([550.0, 350.0]), "the text run does not");
}

#[test]
fn a_drawn_mask_is_empty_before_its_layer_is_drawn() {
    use libcontrols::LayerShape;
    let mask = LayerMask::drawn(0.0);
    assert!(!mask.contains([0.0, 0.0]), "nothing drawn, nothing inside");
}
