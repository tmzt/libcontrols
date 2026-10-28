//! Immediate-mode UI controls drawn as MSDF primitives.

#![forbid(unsafe_code)]

pub mod arrow_label;
pub mod button;
pub mod control;
pub mod drawer;
pub mod ease;
pub mod edit;
pub mod event;
pub mod filter;
pub mod flags;
pub mod handle;
pub mod label;
pub mod modal;
pub mod options;
pub mod panel;
pub mod radio;
pub mod rect;
pub mod ring;
pub mod router;
pub mod scroll;
pub mod selectable_label;
pub mod slider;
pub mod slider_numeric;
pub mod symbol;
pub mod text_area;
pub mod theme;
pub mod tooltip;
pub mod traits;

pub use arrow_label::{ArrowLabel, ArrowLabelActive, ArrowLabelState};
pub use button::{Button, ButtonState};
pub use control::{Control, HostedControl};
pub use drawer::{
    Drawer, DrawerEdge, DrawerEvent, DrawerLayers, DrawerLayout, DrawerOptions, DrawerPhase,
    DrawerReason, DrawerResolved, DrawerRoute, DrawerSize, DrawerState, DrawerTheme, LayerPlan,
    MainUpdates,
};
pub use ease::{CubicBezier, hermite_ease};
pub use edit::{Edit, EditState};
pub use event::{
    ControlAction, ControlPart, EventContext, FocusEvent, GestureEvent, InputEvent, Key, KeyState,
    KeyboardEvent, Modifiers, MouseButton, MouseEvent,
};
pub use filter::{FilterContext, FilterStack, Filtered, InputFilter};
pub use flags::{NodeFlags, NodeState};
pub use handle::{ControlHandle, ReadValue};
pub use label::{Label, LabelState};
pub use modal::{
    Backdrop, ContentReveal, DismissOn, DismissReason, LayerContext, LayerFilter, LayerMask,
    LayerShape, LayerView, Modal, ModalDialog, ModalEntryView, ModalError, ModalEvent, ModalFrame,
    ModalLayerFilter, ModalOptions, ModalPhase, ModalStack, ModalState, ModalTransition, ModalView,
    PanelFrame, ShapedLayerFilter,
};
pub use options::{DEFAULT_PER_ROW, DEFAULT_ROW_GAP, MAX_OPTIONS, Options, OptionsState};
pub use panel::{Panel, PanelData, PanelState};
pub use radio::{DEFAULT_CELL_GAP, DEFAULT_CELL_WIDTH, NO_CELL, Radio, RadioState};
pub use rect::{Rect, TextAlign};
pub use ring::{GlowRingStyle, draw_glowing_ring};
pub use router::{DragGrab, EventResponse, NameError, NodeId, ParseNodeIdError, Router};
pub use scroll::{Overscroll, ScrollAxis, ScrollEdge, ScrollPane, ScrollState};
pub use selectable_label::{SelectableLabel, SelectableLabelState};
pub use slider::{Slider, SliderState, ThumbStyle};
pub use slider_numeric::{NumericFormat, SliderNumeric, SliderNumericActive, SliderNumericState};
pub use symbol::Symbol;
pub use text_area::{TextArea, TextAreaState};
pub use theme::{ControlTheme, ScrollTheme};
pub use tooltip::{LABEL_SEPARATOR, TooltipStyle, draw_tooltip, split_label};
pub use traits::{DrawContext, DrawControl};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use libmsdf::drawlist::{DrawList, SdfKind};

    #[test]
    fn label_draws_text_and_active_glowing_ring() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let label = Label::new("Volume", Rect::new(20.0, 20.0, 120.0, 32.0))
            .with_bg([0.1, 0.1, 0.1, 1.0])
            .with_active(true);

        let handle = label.handle();
        let val: String = handle.read();
        assert_eq!(val, "Volume");

        label.draw(&mut list, &ctx);

        assert!(list.instances.len() >= 3);
        let has_outline = list
            .instances
            .iter()
            .any(|inst| matches!(inst.kind, SdfKind::Outline { .. }));
        assert!(has_outline, "active label must emit glowing outline rings");
    }

    #[test]
    fn button_draws_pressed_state_and_emits_actions() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let btn = Button::new("MODE: AUTO", Rect::new(10.0, 10.0, 120.0, 28.0))
            .with_active(true)
            .with_pressed(true);

        let handle = btn.handle();
        let text: String = handle.read();
        let is_pressed: bool = handle.read();
        assert_eq!(text, "MODE: AUTO");
        assert!(is_pressed);

        btn.draw(&mut list, &ctx);
        assert!(list.instances.len() >= 3);

        let mut router = Router::new();
        let toggle = router.add_named(None, "toggle_btn", btn.clone()).unwrap();
        assert_eq!(router.find("toggle_btn"), Some(toggle));

        let click_resp = router.deliver_mouse(MouseEvent::Click {
            pos: [30.0, 20.0],
            button: MouseButton::Left,
        });
        assert!(click_resp.handled);
        assert_eq!(click_resp.target, Some(toggle));
        assert_eq!(
            click_resp.action,
            Some(ControlAction::Activated {
                part: ControlPart::Body
            })
        );
    }

    #[test]
    fn arrow_label_renders_chevrons_and_sub_rects() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let arrow_lbl = ArrowLabel::new("Option B", Rect::new(0.0, 0.0, 180.0, 36.0))
            .with_active(ArrowLabelActive::Value);

        let handle = arrow_lbl.handle();
        let val: String = handle.read();
        assert_eq!(val, "Option B");

        let (left_r, center_r, right_r) = arrow_lbl.sub_rects();
        assert!(left_r.w > 0.0);
        assert!(center_r.w > 0.0);
        assert!(right_r.w > 0.0);
        assert_eq!(left_r.w + center_r.w + right_r.w, 180.0);

        arrow_lbl.draw(&mut list, &ctx);

        let line_count = list
            .instances
            .iter()
            .filter(|inst| matches!(inst.kind, SdfKind::Line { .. }))
            .count();
        assert_eq!(line_count, 4, "chevrons must emit 4 line segments");
    }

    #[test]
    fn selectable_label_static_options_and_handle_reading() {
        const MODES: &[&str] = &["Option A", "Option B", "Option C", "Option D"];

        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let sel = SelectableLabel::from_static(MODES, Rect::new(0.0, 0.0, 200.0, 36.0))
            .with_selected_index(1)
            .with_active_value(true);

        assert_eq!(sel.selected_index(), 1);
        assert!(sel.left_enabled());
        assert!(sel.right_enabled());
        assert_eq!(sel.display_text(), "Option B");

        let handle = sel.handle();
        let current_str: &'static str = handle.read();
        assert_eq!(current_str, "Option B");

        sel.draw(&mut list, &ctx);
        assert!(!list.instances.is_empty());
    }

    #[test]
    fn slider_fraction_and_active_thumb_glow() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let slider =
            Slider::new(75.0, 0.0, 100.0, Rect::new(10.0, 10.0, 200.0, 24.0)).with_active(true);

        let handle = slider.handle();
        let val: f32 = handle.read();
        assert_eq!(val, 75.0);

        assert_eq!(slider.normalized_fraction(), 0.75);
        let thumb_r = slider.thumb_rect();
        assert!(thumb_r.x > 10.0 && thumb_r.x < 210.0);

        slider.draw(&mut list, &ctx);

        assert!(list.instances.len() >= 5);
    }

    #[test]
    fn an_options_grid_reads_out_as_one_bitfield() {
        const CELLS: &[&str] = &["NW", "OP", "SV", "PR", "CL", "UN", "RE"];

        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let options = Options::new(CELLS, Rect::new(10.0, 20.0, 300.0, 18.0)).with_active(true);
        assert_eq!(options.bits(), 0, "nothing is on to begin with");
        assert_eq!(options.rows(), 2);
        assert_eq!(options.grid_height(), 39.0, "two rows of 18 and a gap of 3");
        assert_eq!(options.packed_width(), 162.0, "five cells of 30, four gaps");
        assert_eq!(options.mask(), 0b111_1111);

        options.draw(&mut list, &ctx);
        let outlines = list
            .instances
            .iter()
            .filter(|inst| matches!(inst.kind, SdfKind::Outline { .. }))
            .count();
        assert_eq!(outlines, 9, "seven cell borders and the two glow rings");

        let mut router = Router::new();
        let wear = router.add_named(None, "wear", options.clone()).unwrap();

        for (pos, index) in [([90.0, 29.0], 2u32), ([20.0, 50.0], 5)] {
            let pressed = router.deliver_mouse(MouseEvent::Click {
                pos,
                button: MouseButton::Left,
            });
            assert!(pressed.handled, "cell {index} must take the click");
            assert_eq!(pressed.target, Some(wear));
            assert_eq!(pressed.target_part, Some(ControlPart::Custom(index)));
        }
        assert_eq!(options.bits(), 0b10_0100, "cells 2 and 5, independently");

        let handle = options.handle();
        let bits: u32 = handle.read();
        let cursor: usize = handle.read();
        let label: &'static str = handle.read();
        assert_eq!(bits, 0b10_0100);
        assert_eq!((cursor, label), (5, "UN"), "the cursor follows the press");

        router.deliver_mouse(MouseEvent::Click {
            pos: [90.0, 29.0],
            button: MouseButton::Left,
        });
        assert_eq!(options.bits(), 0b10_0000);
    }

    #[test]
    fn an_options_grid_steps_by_row_and_toggles_under_the_cursor() {
        const CELLS: &[&str] = &["NW", "OP", "SV", "PR", "CL", "UN", "RE"];

        let options = Options::new(CELLS, Rect::new(0.0, 0.0, 300.0, 18.0));
        let mut router = Router::new();
        let wear = router.add_named(None, "wear", options.clone()).unwrap();
        router.set_focus(Some(wear));

        let key = |key: Key| KeyboardEvent::KeyDown { key };
        for (press, at) in [
            (Key::Right, 1usize),
            (Key::Down, 6),
            (Key::Up, 1),
            (Key::Right, 2),
            (Key::Left, 1),
        ] {
            assert!(router.deliver_keyboard(key(press)).handled);
            assert_eq!(options.cursor(), at, "{press:?} must step to {at}");
            assert_eq!(options.bits(), 0, "stepping lights nothing");
        }

        options.set_cursor(0);
        for press in [Key::Left, Key::Up] {
            assert_eq!(router.deliver_keyboard(key(press)).action, None);
            assert_eq!(options.cursor(), 0, "{press:?} must stop at the start");
        }
        options.set_cursor(CELLS.len() - 1);
        for press in [Key::Right, Key::Down] {
            assert_eq!(router.deliver_keyboard(key(press)).action, None);
            assert_eq!(
                options.cursor(),
                CELLS.len() - 1,
                "{press:?} must stop at the end",
            );
        }

        for bits in [1u32 << (CELLS.len() - 1), 0] {
            assert!(router.deliver_keyboard(key(Key::Enter)).handled);
            assert_eq!(options.bits(), bits);
        }
    }

    #[test]
    fn a_bit_without_a_cell_is_dropped() {
        const CELLS: &[&str] = &["NW", "OP"];
        let options = Options::new(CELLS, Rect::new(0.0, 0.0, 100.0, 18.0));
        options.set_bits(0b1011);
        assert_eq!(options.bits(), 0b11);
        assert!(options.is_on(0) && options.is_on(1) && !options.is_on(3));

        options.set_on(5, true);
        assert_eq!(options.bits(), 0b11);
        options.set_cursor(5);
        assert_eq!(options.cursor(), 1);
        assert_eq!(options.cell_at([80.0, 9.0]), None, "the empty remainder");
        assert_eq!(Options::bit(1), 2);
        assert_eq!(Options::bit(MAX_OPTIONS), 0, "no bit to give");
    }

    #[test]
    fn a_radio_strip_lights_the_cell_that_was_pressed() {
        const CELLS: &[&str] = &["CS", "HD", "AR", "LG", "PS", "DM"];

        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let radio = Radio::new(CELLS, Rect::new(10.0, 20.0, 300.0, 18.0)).with_active(true);
        assert_eq!(radio.selected_index(), 0);
        assert_eq!(radio.packed_width(), 195.0);

        radio.draw(&mut list, &ctx);
        assert!(list.instances.len() >= 14);
        let outlines = list
            .instances
            .iter()
            .filter(|inst| matches!(inst.kind, SdfKind::Outline { .. }))
            .count();
        assert_eq!(outlines, 8, "six cell borders and the two glow rings");

        let mut router = Router::new();
        let tabs = router.add_named(None, "tabs", radio.clone()).unwrap();

        let pressed = router.deliver_mouse(MouseEvent::Click {
            pos: [90.0, 29.0],
            button: MouseButton::Left,
        });
        assert!(pressed.handled);
        assert_eq!(pressed.target, Some(tabs));
        assert_eq!(pressed.target_part, Some(ControlPart::Custom(2)));
        assert_eq!(
            pressed.action,
            Some(ControlAction::Activated {
                part: ControlPart::Custom(2)
            })
        );

        assert_eq!(radio.selected_index(), 2);
        let handle = radio.handle();
        let index: usize = handle.read();
        let label: &'static str = handle.read();
        assert_eq!(index, 2);
        assert_eq!(label, "AR");
    }

    #[test]
    fn a_press_that_lands_on_no_cell_selects_none_of_them() {
        const CELLS: &[&str] = &["CS", "HD", "AR", "LG", "PS", "DM"];

        let radio = Radio::new(CELLS, Rect::new(10.0, 20.0, 300.0, 18.0)).with_selected_index(3);
        let mut router = Router::new();
        router.add(None, radio.clone());

        let gap = router.deliver_mouse(MouseEvent::Click {
            pos: [41.5, 29.0],
            button: MouseButton::Left,
        });
        assert!(!gap.handled, "the gap between two cells is not a cell");

        assert!(radio.bounds().contains([250.0, 29.0]));
        let blank = router.deliver_mouse(MouseEvent::Click {
            pos: [250.0, 29.0],
            button: MouseButton::Left,
        });
        assert!(!blank.handled, "the empty end of the row is not a cell");

        assert_eq!(radio.selected_index(), 3, "and neither moved the selection");
        assert_eq!(radio.cell_at([41.5, 29.0]), None);
        assert_eq!(radio.cell_at([250.0, 29.0]), None);
        assert_eq!(radio.cell_at([90.0, 29.0]), Some(2));
    }

    #[test]
    fn the_arrow_keys_walk_the_selection_and_stop_at_the_ends() {
        const CELLS: &[&str] = &["CS", "HD", "AR"];

        let radio = Radio::new(CELLS, Rect::new(0.0, 0.0, 200.0, 18.0));
        let mut router = Router::new();
        let id = router.add(None, radio.clone());
        router.set_focus(Some(id));

        let stop_left = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
        assert!(
            stop_left.action.is_none(),
            "the first cell has nothing to its left"
        );
        assert_eq!(radio.selected_index(), 0);

        for expected in 1..=2 {
            let step = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
            assert!(step.handled);
            assert_eq!(step.action, Some(ControlAction::StepRight));
            assert_eq!(radio.selected_index(), expected);
        }

        let stop_right = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
        assert!(
            stop_right.action.is_none(),
            "the last cell has nothing to its right"
        );
        assert_eq!(radio.selected_index(), 2);

        let back = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
        assert_eq!(back.action, Some(ControlAction::StepLeft));
        assert_eq!(radio.selected_index(), 1);

        let space = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Space });
        assert_eq!(
            space.action,
            Some(ControlAction::Activated {
                part: ControlPart::Custom(1)
            })
        );
        assert_eq!(radio.selected_index(), 1);
    }

    #[test]
    fn a_radio_strip_shares_its_state_the_way_the_other_controls_do() {
        const CELLS: &[&str] = &["CS", "HD", "AR"];

        let state = Arc::new(RadioState::new(0));
        let radio = Radio::new(CELLS, Rect::new(0.0, 0.0, 200.0, 18.0))
            .with_state(Arc::clone(&state))
            .with_cells(40.0, 4.0);

        let mut router = Router::new();
        router.add(None, radio);
        router.deliver_mouse(MouseEvent::Click {
            pos: [60.0, 9.0],
            button: MouseButton::Left,
        });
        assert_eq!(
            state.selected_index(),
            1,
            "the cell from 44 to 84 is the second"
        );

        let rebuilt = Radio::new(CELLS, Rect::new(0.0, 0.0, 200.0, 18.0))
            .with_state(Arc::clone(&state))
            .with_cells(40.0, 4.0);
        assert_eq!(rebuilt.selected_index(), 1);
        assert_eq!(rebuilt.selected_label(), "HD");

        rebuilt.set_selected_index(9);
        assert_eq!(state.selected_index(), 2);
        assert_eq!(rebuilt.selected_label(), "AR");
    }

    #[test]
    fn slider_numeric_formats_and_partitions() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let sn = SliderNumeric::new(1.456, 0.0, 3.0, Rect::new(0.0, 0.0, 240.0, 32.0))
            .with_label("Height")
            .with_format(NumericFormat::Decimal { places: 2 })
            .with_active(SliderNumericActive::Value);

        let handle = sn.handle();
        let val_num: f32 = handle.read();
        assert_eq!(val_num, 1.456);

        let val_str: String = handle.read();
        assert_eq!(val_str, "1.46");

        let (lbl_r, slider_r, num_r) = sn.sub_rects();
        assert!(lbl_r.is_some());
        assert!(slider_r.w > 0.0);
        assert!(num_r.w > 0.0);

        sn.draw(&mut list, &ctx);
        assert!(!list.instances.is_empty());
    }

    #[test]
    fn router_add_and_mouse_event_delivery() {
        let label = Label::new("Settings", Rect::new(10.0, 10.0, 300.0, 40.0));
        let sel = SelectableLabel::from_static(
            &["Option A", "Option B", "Option C"],
            Rect::new(10.0, 60.0, 300.0, 36.0),
        )
        .with_selected_index(1);
        let slider = Slider::new(50.0, 0.0, 100.0, Rect::new(10.0, 110.0, 300.0, 24.0));
        let num_slider = SliderNumeric::new(1.20, 0.5, 2.0, Rect::new(10.0, 150.0, 300.0, 32.0))
            .with_label("Scale");

        let mut router = Router::new();
        router.add_named(None, "title", label).unwrap();
        let mode_select = router.add_named(None, "mode_select", sel).unwrap();
        let volume_slider = router.add_named(None, "volume_slider", slider).unwrap();
        router.add_named(None, "scale_slider", num_slider).unwrap();

        assert_eq!(router.len(), 4);
        assert_eq!(router.find("volume_slider"), Some(volume_slider));

        let resp_left = router.deliver_mouse(MouseEvent::Click {
            pos: [15.0, 70.0],
            button: MouseButton::Left,
        });
        assert!(resp_left.handled);
        assert_eq!(resp_left.target, Some(mode_select));
        assert_eq!(resp_left.target_part, Some(ControlPart::LeftArrow));
        assert_eq!(resp_left.action, Some(ControlAction::StepLeft));
        assert_eq!(router.focused(), Some(mode_select));

        let resp_slider = router.deliver_mouse(MouseEvent::Click {
            pos: [235.0, 120.0],
            button: MouseButton::Left,
        });
        assert!(resp_slider.handled);
        assert_eq!(resp_slider.target, Some(volume_slider));
        if let Some(ControlAction::ValueChange {
            new_value,
            fraction,
        }) = resp_slider.action
        {
            assert!((fraction - 0.75).abs() < 0.01);
            assert!((new_value - 75.0).abs() < 1.0);
        } else {
            panic!("Expected ValueChange action for slider click");
        }
        assert_eq!(router.focused(), Some(volume_slider));

        let resp_outside = router.deliver_mouse(MouseEvent::Click {
            pos: [400.0, 400.0],
            button: MouseButton::Left,
        });
        assert!(!resp_outside.handled);
        assert_eq!(router.focused(), None);
    }

    #[test]
    fn a_drag_keeps_hold_of_its_slider_across_frames() {
        let top = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let below = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut router = Router::new();
        let top_id = router.add(None, top.clone());
        router.add(None, below.clone());

        let down = router.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target, Some(top_id), "the press lands on the top one");
        let grab = router.active_drag().expect("the press must grab it");
        assert_eq!(grab.node, top_id);

        // A frame goes by: the router persists, and so does its grab.
        let mut list = DrawList::new();
        router.render(&mut list, &DrawContext::default());
        assert_eq!(router.active_drag(), Some(grab), "the grab must survive");

        let moved = router.deliver_mouse(MouseEvent::Move { pos: [180.0, 38.0] });
        assert_eq!(
            moved.target,
            Some(top_id),
            "the drag keeps the slider it grabbed, not the one under the pointer",
        );
        assert!(top.value() > 0.8, "and the one it grabbed is what moved");
        assert!(
            (below.value() - 0.5).abs() < 1e-6,
            "the one it wandered over must not have moved at all",
        );

        router.deliver_mouse(MouseEvent::Up {
            pos: [180.0, 38.0],
            button: MouseButton::Left,
        });
        assert!(router.active_drag().is_none(), "the release must let go");
    }

    #[test]
    fn sweeping_across_sliders_with_nothing_grabbed_moves_none_of_them() {
        let top = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let below = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));
        let mut router = Router::new();
        router.add(None, top.clone());
        router.add(None, below.clone());

        router.deliver_mouse(MouseEvent::Move { pos: [190.0, 10.0] });
        router.deliver_mouse(MouseEvent::Move { pos: [190.0, 38.0] });
        assert_eq!(
            top.value(),
            0.5,
            "a move with nothing grabbed moves nothing"
        );
        assert_eq!(below.value(), 0.5, "and nothing means nothing");
    }

    #[test]
    fn a_cleared_focus_stays_cleared() {
        let mut router = Router::new();
        let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 200.0, 60.0)));
        let one = router.add(
            Some(panel),
            Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0)),
        );
        router.add(
            Some(panel),
            Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0)),
        );
        router.set_focus(Some(one));
        router.set_focus(None);
        assert!(
            !router
                .deliver_event(InputEvent::Keyboard(KeyboardEvent::KeyDown {
                    key: Key::Space
                }))
                .handled,
            "with nothing focused a key press has nowhere to go",
        );
    }

    #[test]
    fn escape_reports_that_it_cleared_the_focus() {
        let mut router = Router::new();
        let only = router.add(
            None,
            Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0)),
        );
        router.set_focus(Some(only));
        assert_eq!(router.focused(), Some(only));
        let escaped = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
        assert!(escaped.handled, "the clear must be reported");
        assert_eq!(escaped.focused, None);
        assert_eq!(router.focused(), None);
    }

    #[test]
    fn a_drag_grab_naming_a_control_that_is_gone_is_dropped() {
        let mut router = Router::new();
        let only = router.add(
            None,
            Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0)),
        );
        router.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(router.active_drag().map(|g| g.node), Some(only));

        router.remove(only);
        assert!(
            router.active_drag().is_none(),
            "a grab on nothing is no grab"
        );
    }

    #[test]
    fn a_focus_key_whose_control_is_gone_does_not_land_on_its_successor() {
        let level = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let leg = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let tab = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut router = Router::new();
        router.add_named(None, "tab", tab).unwrap();
        let level_id = router.add_named(None, "level_slider", level).unwrap();
        router.set_focus(Some(level_id));
        assert_eq!(router.focused(), Some(level_id));

        router.remove(level_id);
        assert_eq!(
            router.focused(),
            None,
            "the control the focus named is gone, so the focus is gone with it",
        );

        // The successor may take the removed node's slot, never its id.
        let leg_id = router.add_named(None, "leg_slider", leg).unwrap();
        assert_ne!(leg_id, level_id);
        assert_eq!(router.focused(), None, "the successor is not focused");
        router.set_focus(Some(level_id));
        assert_eq!(
            router.focused(),
            None,
            "an old id names nothing, not whatever stands in its place",
        );
        // The rebuild-era "a place is a place" half is gone: the router has
        // no positional keys.
    }

    #[test]
    fn a_drag_grab_whose_control_is_gone_lets_go() {
        let level = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let leg = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let tab = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut router = Router::new();
        router.add_named(None, "tab", tab).unwrap();
        let level_id = router.add_named(None, "level_slider", level).unwrap();
        let down = router.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target, Some(level_id));
        assert!(router.active_drag().is_some(), "the press must grab it");

        router.remove(level_id);
        router.add_named(None, "leg_slider", leg.clone()).unwrap();
        assert!(
            router.active_drag().is_none(),
            "the control it had hold of is gone, so it is holding nothing",
        );

        router.deliver_mouse(MouseEvent::Move { pos: [190.0, 10.0] });
        assert_eq!(
            leg.value(),
            0.5,
            "and the slider standing where it was must not have moved",
        );
        // The rebuild-era half that restored a grab by position is gone: the
        // router has no positional keys.
    }

    #[test]
    fn a_node_is_still_found_by_name_after_the_tree_changes() {
        let alpha = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let beta = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));
        // Drawn over beta, so a plain hit at beta's place would find it.
        let inserted = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut router = Router::new();
        let alpha_id = router.add_named(None, "alpha", alpha).unwrap();
        let beta_id = router.add_named(None, "beta", beta.clone()).unwrap();
        let down = router.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 38.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target, Some(beta_id));
        assert_eq!(router.focused(), Some(beta_id), "beta has the focus");
        assert!(
            router.active_drag().is_some(),
            "and the press has hold of it"
        );

        router.remove(alpha_id);
        router
            .add_named(None, "inserted", inserted.clone())
            .unwrap();
        assert_eq!(router.find("beta"), Some(beta_id), "beta is still here");
        assert_eq!(router.focused(), Some(beta_id), "and still focused");

        let moved = router.deliver_mouse(MouseEvent::Move { pos: [190.0, 38.0] });
        assert_eq!(moved.target, Some(beta_id));
        assert!(beta.value() > 0.8, "and the drag still has hold of it");
        assert_eq!(
            inserted.value(),
            0.5,
            "not of whatever now sits where it used to",
        );
    }

    #[test]
    fn router_keyboard_event_delivery_and_focus_cycling() {
        let label = Label::new("Header", Rect::new(0.0, 0.0, 200.0, 30.0));
        let sel = SelectableLabel::from_static(
            &["Option 1", "Option 2"],
            Rect::new(0.0, 40.0, 200.0, 30.0),
        );
        let slider = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 80.0, 200.0, 30.0));

        let mut router = Router::new();
        router.add(None, label);
        let sel_id = router.add(None, sel);
        let slider_id = router.add(None, slider);

        assert_eq!(router.focused(), None);

        let tab_resp = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Tab });
        assert!(tab_resp.handled);
        assert_eq!(router.focused(), Some(sel_id), "a label is no Tab stop");

        let right_resp = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
        assert!(right_resp.handled);
        assert_eq!(right_resp.action, Some(ControlAction::StepRight));

        router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Tab });
        assert_eq!(router.focused(), Some(slider_id));

        let slider_step = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
        assert!(slider_step.handled);
        if let Some(ControlAction::ValueChange {
            new_value,
            fraction: _,
        }) = slider_step.action
        {
            assert!((new_value - 0.52).abs() < 0.001);
        } else {
            panic!("Expected ValueChange on slider step");
        }

        let backtab_resp = router.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
        assert!(backtab_resp.handled);
        assert_eq!(router.focused(), Some(sel_id));
    }

    #[test]
    fn router_mutates_underlying_control_states_directly() {
        let mode_state = Arc::new(SelectableLabelState::new(0));
        let sel = SelectableLabel::from_static(
            &["Option A", "Option B", "Option C"],
            Rect::new(0.0, 0.0, 200.0, 30.0),
        )
        .with_state(Arc::clone(&mode_state));

        let slider_state = Arc::new(SliderState::new(0.5));
        let slider = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 40.0, 200.0, 30.0))
            .with_state(Arc::clone(&slider_state));

        let btn_state = Arc::new(ButtonState::new(false));
        let btn = Button::new("AUTO", Rect::new(0.0, 80.0, 200.0, 30.0))
            .with_state(Arc::clone(&btn_state));

        let mut router = Router::new();
        let sel_id = router.add(None, sel);
        let slider_id = router.add(None, slider);
        let btn_id = router.add(None, btn);

        router.deliver_mouse(MouseEvent::Click {
            pos: [190.0, 15.0],
            button: MouseButton::Left,
        });
        assert_eq!(mode_state.selected_index(), 1);
        let sel = router.get::<SelectableLabel<&'static str>>(sel_id).unwrap();
        assert_eq!(sel.selected_index(), 1);

        router.deliver_mouse(MouseEvent::Down {
            pos: [160.0, 55.0],
            button: MouseButton::Left,
        });
        assert!((slider_state.value() - 0.8).abs() < 0.01);
        let slider = router.get::<Slider>(slider_id).unwrap();
        assert!((slider.value() - 0.8).abs() < 0.01);
        router.deliver_mouse(MouseEvent::Up {
            pos: [160.0, 55.0],
            button: MouseButton::Left,
        });

        router.deliver_mouse(MouseEvent::Click {
            pos: [100.0, 95.0],
            button: MouseButton::Left,
        });
        assert!(btn_state.is_pressed());
        assert!(router.get::<Button>(btn_id).unwrap().is_pressed());
    }

    #[test]
    fn edit_control_renders_and_reads_value() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let edit = Edit::new("42.5", Rect::new(10.0, 10.0, 60.0, 24.0)).with_active(true);

        let handle = edit.handle();
        let text: String = handle.read();
        assert_eq!(text, "42.5");

        edit.draw(&mut list, &ctx);
        assert!(list.instances.len() >= 3);
    }

    #[test]
    fn slider_numeric_composes_slider_edit_and_label() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let sn = SliderNumeric::new(0.75, 0.0, 1.0, Rect::new(0.0, 0.0, 240.0, 32.0))
            .with_label("Volume")
            .with_format(NumericFormat::Percentage)
            .with_active(SliderNumericActive::Value);

        assert!(sn.label.is_some());
        assert_eq!(sn.slider.value(), 0.75);
        assert_eq!(sn.edit.text(), "75%");

        let handle = sn.handle();
        let val_num: f32 = handle.read();
        let val_str: String = handle.read();
        assert_eq!(val_num, 0.75);
        assert_eq!(val_str, "75%");

        sn.draw(&mut list, &ctx);
        assert!(!list.instances.is_empty());

        let mut router = Router::new();
        router.add_named(None, "sn", sn.clone()).unwrap();

        let (_, slider_r, _) = sn.sub_rects();
        let click_x = slider_r.x + slider_r.w * 0.5;
        let click_y = slider_r.y + slider_r.h * 0.5;
        router.deliver_mouse(MouseEvent::Click {
            pos: [click_x, click_y],
            button: MouseButton::Left,
        });

        assert!((sn.value() - 0.5).abs() < 0.02);
        assert_eq!(sn.edit.text(), "50%");
    }

    #[test]
    fn panel_renders_background_border_and_headers() {
        let mut list = DrawList::new();
        let ctx = DrawContext::default();

        let vp_panel = Panel::new(Rect::new(0.0, 0.0, 300.0, 600.0))
            .with_title("VIEWPORT")
            .with_border([0.18, 0.72, 0.98, 0.40], 1.2)
            .with_corner_radius(8.0);

        assert_eq!(vp_panel.title().as_deref(), Some("VIEWPORT"));
        assert!(vp_panel.bg_color.is_none());
        assert!(vp_panel.border_color.is_some());

        vp_panel.draw(&mut list, &ctx);
        let has_box = list
            .instances
            .iter()
            .any(|inst| matches!(inst.kind, SdfKind::Box | SdfKind::RoundedBox { .. }));
        assert!(!has_box, "transparent panel must NOT emit background boxes");

        let hud_panel = Panel::new(Rect::new(310.0, 0.0, 300.0, 600.0))
            .with_title("HUD")
            .with_subtitle("SUBTITLE")
            .with_bg([0.04, 0.06, 0.10, 0.94])
            .with_border([0.18, 0.72, 0.98, 0.50], 1.2);

        hud_panel.draw(&mut list, &ctx);
        let has_hud_box = list
            .instances
            .iter()
            .any(|inst| matches!(inst.kind, SdfKind::RoundedBox { .. }));
        assert!(has_hud_box, "HUD panel must emit background box");

        let handle = hud_panel.handle();
        let title_val: String = handle.read();
        assert_eq!(title_val, "HUD");
    }

    #[test]
    fn panel_delivers_events_to_child_controls() {
        let button_state = Arc::new(ButtonState::new(false));
        let button = Button::new("TOGGLE", Rect::new(20.0, 20.0, 120.0, 30.0))
            .with_state(Arc::clone(&button_state));
        let mut router = Router::new();
        let panel = router.add(None, Panel::new(Rect::new(0.0, 0.0, 200.0, 100.0)));
        let toggle = router.add_named(Some(panel), "toggle", button).unwrap();

        let response = router.deliver_event(InputEvent::Mouse(MouseEvent::Click {
            pos: [40.0, 30.0],
            button: MouseButton::Left,
        }));

        assert!(response.handled);
        assert_eq!(response.target, Some(toggle), "the child, not the panel");
        assert!(button_state.is_pressed());
    }

    #[derive(Clone, Debug, Default, PartialEq)]
    struct Properties {
        enabled: bool,
        label: String,
    }

    #[test]
    fn panel_carries_data_edited_by_its_child_controls() {
        let properties = PanelData::new(Properties::default());
        let button_state = Arc::new(ButtonState::new(false));
        let button = Button::new("TOGGLE", Rect::new(20.0, 20.0, 120.0, 30.0))
            .with_state(Arc::clone(&button_state));
        let mut router = Router::new();
        let panel_id = router.add(
            None,
            Panel::new(Rect::new(0.0, 0.0, 200.0, 100.0))
                .with_title("PROPERTIES")
                .with_data(Arc::clone(&properties)),
        );
        router.add(Some(panel_id), button);

        let response = router.deliver_event(InputEvent::Mouse(MouseEvent::Click {
            pos: [40.0, 30.0],
            button: MouseButton::Left,
        }));
        assert!(response.handled);
        let panel = router.get::<Panel<Properties>>(panel_id).unwrap();

        panel.write_data(|properties| {
            properties.enabled = button_state.is_pressed();
            properties.label = "TOGGLE".to_string();
        });

        assert_eq!(
            properties.get(),
            Properties {
                enabled: true,
                label: "TOGGLE".to_string(),
            }
        );
        assert_eq!(panel.read_data(|p| p.enabled), Some(true));
        assert_eq!(panel.title().as_deref(), Some("PROPERTIES"));
    }

    #[test]
    fn stateless_panel_carries_no_data() {
        let panel = Panel::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(panel.data().is_none());
        assert_eq!(panel.read_data(|_: &()| 1), None);
    }

    #[test]
    fn a_label_carries_its_own_tooltip_behind_a_separator() {
        assert_eq!(split_label("PR - Print"), ("PR", Some("Print")));

        assert_eq!(split_label("RESET TO STARTUP"), ("RESET TO STARTUP", None));

        assert_eq!(split_label("X"), ("X", None));
        assert_eq!(split_label(""), ("", None));

        assert_eq!(
            split_label("PR - Print - two-sided"),
            ("PR", Some("Print - two-sided")),
        );

        assert_eq!(split_label(" - Print"), (" - Print", None));

        assert_eq!(split_label("PR - "), ("PR", None));

        assert_eq!(split_label("high-cut"), ("high-cut", None));

        const CELLS: &[&str] = &["SM - Small", "MD - Medium"];
        let radio = Radio::new(CELLS, Rect::new(0.0, 0.0, 300.0, 18.0)).with_abbreviate(true);
        assert_eq!(radio.shown_label(0), "SM");
        assert_eq!(radio.tooltip(0), Some("Small"));
        assert_eq!(radio.selected_label(), "SM - Small");
    }

    #[test]
    fn a_hovered_cell_is_the_one_under_the_pointer_and_nothing_else() {
        const CELLS: &[&str] = &["SM - Small", "MD - Medium", "LG - Large"];
        let radio = Radio::new(CELLS, Rect::new(10.0, 20.0, 300.0, 18.0));
        let mut router = Router::new();
        router.add_named(None, "cycle", radio.clone()).unwrap();

        assert_eq!(radio.hovered_cell(), None, "nothing is hovered to start");

        let moved = router.deliver_mouse(MouseEvent::Move { pos: [90.0, 29.0] });
        assert!(!moved.handled, "a hover is not an interaction");
        assert_eq!(radio.hovered_cell(), Some(2));
        assert_eq!(radio.selected_index(), 0, "and it selects nothing");

        router.deliver_mouse(MouseEvent::Move { pos: [41.5, 29.0] });
        assert_eq!(radio.hovered_cell(), None);

        router.deliver_mouse(MouseEvent::Move { pos: [90.0, 29.0] });
        assert_eq!(radio.hovered_cell(), Some(2));

        router.deliver_mouse(MouseEvent::Move { pos: [90.0, 300.0] });
        assert_eq!(radio.hovered_cell(), None);
    }

    #[test]
    fn hovering_one_control_clears_the_hover_on_another() {
        const CELLS: &[&str] = &["SV - Save", "PR - Print"];
        let grid = Options::new(CELLS, Rect::new(0.0, 0.0, 200.0, 20.0)).with_abbreviate(true);
        let button =
            Button::new("CT - Center", Rect::new(0.0, 40.0, 60.0, 20.0)).with_abbreviate(true);
        let mut router = Router::new();
        router.add(None, grid.clone());
        router.add(None, button.clone());

        router.deliver_mouse(MouseEvent::Move { pos: [15.0, 10.0] });
        assert_eq!(grid.hovered_cell(), Some(0));
        assert!(!button.is_hovered());
        assert_eq!(grid.tooltip(0), Some("Save"));

        router.deliver_mouse(MouseEvent::Move { pos: [30.0, 50.0] });
        assert!(button.is_hovered(), "the button has it now");
        assert_eq!(grid.hovered_cell(), None, "and the grid has let go");
        assert_eq!(button.shown_text(), "CT");
        assert_eq!(button.tooltip(), Some("Center"));

        router.deliver_mouse(MouseEvent::Down {
            pos: [15.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(grid.hovered_cell(), Some(0));
        assert!(!button.is_hovered());
    }

    #[test]
    fn a_tooltip_is_drawn_over_the_controls_that_follow_it() {
        const CELLS: &[&str] = &["SV - Save", "PR - Print"];
        const TIP_BG: [f32; 4] = [0.5, 0.0, 0.5, 1.0];
        const BUTTON_BG: [f32; 4] = [0.0, 0.5, 0.0, 1.0];

        let grid = Options::new(CELLS, Rect::new(0.0, 0.0, 200.0, 20.0))
            .with_abbreviate(true)
            .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
        let button = Button::new("CT", Rect::new(0.0, 40.0, 60.0, 20.0)).with_bg(BUTTON_BG);
        let mut router = Router::new();
        router.add(None, grid);
        router.add(None, button);

        let ctx = DrawContext::default();
        let mut quiet = DrawList::new();
        router.render(&mut quiet, &ctx);
        assert!(
            !quiet.instances.iter().any(|inst| inst.color == TIP_BG),
            "nothing hovered draws no tip",
        );

        router.deliver_mouse(MouseEvent::Move { pos: [15.0, 10.0] });
        let mut list = DrawList::new();
        router.render(&mut list, &ctx);

        let tip = list
            .instances
            .iter()
            .position(|inst| inst.color == TIP_BG)
            .expect("the hovered cell must draw its tip");
        let behind = list
            .instances
            .iter()
            .position(|inst| inst.color == BUTTON_BG)
            .expect("the button is drawn too");
        assert!(
            tip > behind,
            "the tip must be pushed after the control registered behind it",
        );
    }

    #[test]
    fn a_tooltip_stays_inside_the_frame_it_is_given() {
        let frame = Rect::new(100.0, 100.0, 200.0, 200.0);
        let style = TooltipStyle {
            padding: 0.0,
            gap: 2.0,
            ..TooltipStyle::default()
        }
        .within(frame);

        let cell = Rect::new(150.0, 120.0, 30.0, 20.0);
        let below = style.place(cell, [40.0, 12.0]);
        assert_eq!(below.y, 142.0);
        assert_eq!(below.x, 145.0);

        let low = Rect::new(150.0, 280.0, 30.0, 20.0);
        assert_eq!(style.place(low, [40.0, 12.0]).y, 266.0);

        let left = Rect::new(100.0, 120.0, 30.0, 20.0);
        assert_eq!(style.place(left, [60.0, 12.0]).x, 100.0);
        let right = Rect::new(270.0, 120.0, 30.0, 20.0);
        assert_eq!(style.place(right, [60.0, 12.0]).x, 240.0);

        let loose = TooltipStyle {
            padding: 0.0,
            gap: 2.0,
            ..TooltipStyle::default()
        };
        assert_eq!(loose.place(low, [40.0, 12.0]).y, 302.0);
    }

    #[test]
    fn abbreviation_is_opt_in_on_the_multi_cell_controls() {
        const NAMED: &[&str] = &["NW - New", "OP - Open", "XY"];
        let rect = Rect::new(0.0, 0.0, 300.0, 18.0);

        let radio = Radio::new(NAMED, rect);
        assert!(!radio.abbreviate, "off by default");
        assert_eq!(radio.shown_label(0), "NW - New", "drawn as written");
        assert_eq!(radio.tooltip(0), None);
        let options = Options::new(NAMED, rect);
        assert!(!options.abbreviate, "off by default");
        assert_eq!(options.shown_label(1), "OP - Open", "drawn as written");
        assert_eq!(options.tooltip(1), None);
        assert_eq!(radio.shown_label(2), "XY");
        assert_eq!(options.shown_label(2), "XY");

        let radio = Radio::new(NAMED, rect).with_abbreviate(true);
        assert_eq!(radio.shown_label(0), "NW");
        assert_eq!(radio.tooltip(0), Some("New"));
        let options = Options::new(NAMED, rect).with_abbreviate(true);
        assert_eq!(options.shown_label(1), "OP");
        assert_eq!(options.tooltip(1), Some("Open"));
        assert_eq!(radio.shown_label(2), "XY");
        assert_eq!(radio.tooltip(2), None);
        assert_eq!(options.shown_label(2), "XY");
        assert_eq!(options.tooltip(2), None);
    }

    /// Tooltip background used to find a drawn tip.
    const TIP_BG: [f32; 4] = [0.5, 0.0, 0.5, 1.0];

    /// Whether `control`, hovered at `pos` in a router, draws a tooltip box.
    fn draws_tip_when_hovered<C: Control + Clone + 'static>(control: &C, pos: [f32; 2]) -> bool {
        let mut router = Router::new();
        router.add(None, control.clone());
        router.deliver_mouse(MouseEvent::Move { pos });
        let mut list = DrawList::new();
        router.render(&mut list, &DrawContext::default());
        list.instances.iter().any(|inst| inst.color == TIP_BG)
    }

    #[test]
    fn radio_and_options_tooltips_follow_the_setting() {
        const NAMED: &[&str] = &["NW - New", "OP - Open"];
        let rect = Rect::new(0.0, 0.0, 300.0, 18.0);
        let tip = TooltipStyle::default().with_bg(TIP_BG);

        let radio = Radio::new(NAMED, rect).with_tooltip_style(tip);
        assert!(!draws_tip_when_hovered(&radio, [15.0, 9.0]));
        let radio = Radio::new(NAMED, rect)
            .with_tooltip_style(tip)
            .with_abbreviate(true);
        assert!(draws_tip_when_hovered(&radio, [15.0, 9.0]));

        let options = Options::new(NAMED, rect).with_tooltip_style(tip);
        assert!(!draws_tip_when_hovered(&options, [15.0, 9.0]));
        let options = Options::new(NAMED, rect)
            .with_tooltip_style(tip)
            .with_abbreviate(true);
        assert!(draws_tip_when_hovered(&options, [15.0, 9.0]));
    }

    #[test]
    fn a_selectable_label_of_str_draws_as_before() {
        const MODES: &[&str] = &["Option A", "SM - Small"];
        let rect = Rect::new(0.0, 0.0, 200.0, 36.0);
        let sel = SelectableLabel::from_static(MODES, rect);
        assert!(!sel.abbreviate, "off by default");
        assert_eq!(sel.shown_text(), sel.display_text());
        assert_eq!(sel.shown_text(), "Option A");

        let mut drawn = DrawList::new();
        sel.draw(&mut drawn, &DrawContext::default());
        let lines = drawn
            .instances
            .iter()
            .filter(|inst| matches!(inst.kind, SdfKind::Line { .. }))
            .count();
        assert_eq!(lines, 4, "the chevrons as before");
        assert_eq!(drawn.instances.len(), 4, "and nothing else without a font");

        let whole = sel.clone().with_selected_index(1);
        assert_eq!(whole.shown_text(), "SM - Small", "drawn as written");
        assert_eq!(whole.tooltip(), None);
    }

    #[test]
    fn a_selectable_label_of_any_display_type_draws_its_format_fn_text() {
        const SPEEDS: &[u32] = &[1, 2, 4];
        let rect = Rect::new(0.0, 0.0, 200.0, 36.0);

        let plain = SelectableLabel::new(SPEEDS, rect).with_selected_index(2);
        assert_eq!(plain.shown_text(), "4", "Display without a format_fn");

        let sel = SelectableLabel::new(SPEEDS, rect)
            .with_selected_index(1)
            .with_format_fn(|n| format!("{n}x speed"));
        assert_eq!(sel.shown_text(), "2x speed");

        let mut router = Router::new();
        let id = router.add(None, sel.clone());
        let mut list = DrawList::new();
        router.render(&mut list, &DrawContext::default());
        assert!(
            !list.instances.is_empty(),
            "a SelectableLabel<u32> is a Control"
        );
        assert!(router.get::<SelectableLabel<u32>>(id).is_some());
    }

    #[test]
    fn a_button_abbreviates_only_when_asked() {
        let r = Rect::new(0.0, 0.0, 60.0, 20.0);
        let plain = Button::new("CT - Center", r);
        assert_eq!(plain.shown_text(), "CT - Center");
        assert_eq!(plain.tooltip(), None);
        let short = Button::new("CT - Center", r).with_abbreviate(true);
        assert_eq!(short.shown_text(), "CT");
        assert_eq!(short.tooltip(), Some("Center"));
        let bare = Button::new("Center", r).with_abbreviate(true);
        assert_eq!(bare.shown_text(), "Center");
        assert_eq!(bare.tooltip(), None);
    }

    #[test]
    fn a_selectable_label_splits_its_formatted_text_only_when_asked() {
        #[derive(Clone, Debug)]
        enum Sky {
            Clear,
            Rain,
        }
        impl std::fmt::Display for Sky {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{self:?}")
            }
        }
        fn sky_label(sky: &Sky) -> String {
            match sky {
                Sky::Clear => "CS - Clear sky".into(),
                Sky::Rain => "Rain".into(),
            }
        }
        const SKIES: &[Sky] = &[Sky::Clear, Sky::Rain];
        let rect = Rect::new(0.0, 0.0, 200.0, 36.0);
        let tip = TooltipStyle::default().with_bg(TIP_BG);
        let centre = [100.0, 18.0];

        let off = SelectableLabel::new(SKIES, rect)
            .with_format_fn(sky_label)
            .with_tooltip_style(tip);
        assert_eq!(off.shown_text(), "CS - Clear sky", "drawn whole when off");
        assert_eq!(off.tooltip(), None);
        assert!(!draws_tip_when_hovered(&off, centre));

        let on = SelectableLabel::new(SKIES, rect)
            .with_format_fn(sky_label)
            .with_tooltip_style(tip)
            .with_abbreviate(true);
        assert_eq!(on.shown_text(), "CS");
        assert_eq!(on.tooltip().as_deref(), Some("Clear sky"));
        assert!(!on.is_hovered());
        let mut quiet = DrawList::new();
        on.draw_overlay(&mut quiet, &DrawContext::default());
        assert!(quiet.instances.is_empty(), "no tip until hovered");
        assert!(draws_tip_when_hovered(&on, centre), "the name on hover");

        let plain_on = SelectableLabel::new(SKIES, rect)
            .with_format_fn(sky_label)
            .with_abbreviate(true)
            .with_selected_index(1);
        assert_eq!(plain_on.shown_text(), "Rain", "no separator: as written");
        assert_eq!(plain_on.tooltip(), None);
        let plain_off = plain_on.clone().with_abbreviate(false);
        assert_eq!(plain_off.shown_text(), "Rain");
        assert_eq!(plain_off.tooltip(), None);
    }
}
