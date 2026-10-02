//! Immediate-mode UI controls drawn as MSDF primitives.

#![forbid(unsafe_code)]

pub mod arrow_label;
pub mod button;
pub mod edit;
pub mod event;
pub mod handle;
pub mod host;
pub mod label;
pub mod options;
pub mod panel;
pub mod radio;
pub mod rect;
pub mod ring;
pub mod selectable_label;
pub mod slider;
pub mod slider_numeric;
pub mod theme;
pub mod tooltip;
pub mod traits;

pub use arrow_label::{ArrowLabel, ArrowLabelActive, ArrowLabelState};
pub use button::{Button, ButtonState};
pub use edit::{Edit, EditState};
pub use event::{
    ControlAction, ControlHit, ControlPart, EventResponse, InputEvent, Key, KeyState,
    KeyboardEvent, MouseButton, MouseEvent,
};
pub use handle::{ControlHandle, ReadValue};
pub use host::{Control, ControlHost, ControlHostBuilder, ControlKey, DragGrab, HostedControl};
pub use label::{Label, LabelState};
pub use options::{DEFAULT_PER_ROW, DEFAULT_ROW_GAP, MAX_OPTIONS, Options, OptionsState};
pub use panel::{Panel, PanelData, PanelState};
pub use radio::{DEFAULT_CELL_GAP, DEFAULT_CELL_WIDTH, NO_CELL, Radio, RadioState};
pub use rect::{Rect, TextAlign};
pub use ring::{GlowRingStyle, draw_glowing_ring};
pub use selectable_label::{SelectableLabel, SelectableLabelState};
pub use slider::{Slider, SliderState, ThumbStyle};
pub use slider_numeric::{NumericFormat, SliderNumeric, SliderNumericActive, SliderNumericState};
pub use theme::ControlTheme;
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

        let mut host = ControlHost::builder()
            .with_named_control("toggle_btn", btn.handle())
            .build();

        let click_resp = host.deliver_mouse(MouseEvent::Click {
            pos: [30.0, 20.0],
            button: MouseButton::Left,
        });
        assert!(click_resp.handled);
        assert_eq!(click_resp.target_id.as_deref(), Some("toggle_btn"));
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

        let mut host = ControlHost::builder()
            .with_named_control("wear", options.handle())
            .build();

        for (pos, index) in [([90.0, 29.0], 2u32), ([20.0, 50.0], 5)] {
            let pressed = host.deliver_mouse(MouseEvent::Click {
                pos,
                button: MouseButton::Left,
            });
            assert!(pressed.handled, "cell {index} must take the click");
            assert_eq!(pressed.target_id.as_deref(), Some("wear"));
            assert_eq!(pressed.target_part, Some(ControlPart::Custom(index)));
        }
        assert_eq!(options.bits(), 0b10_0100, "cells 2 and 5, independently");

        let handle = options.handle();
        let bits: u32 = handle.read();
        let cursor: usize = handle.read();
        let label: &'static str = handle.read();
        assert_eq!(bits, 0b10_0100);
        assert_eq!((cursor, label), (5, "UN"), "the cursor follows the press");

        host.deliver_mouse(MouseEvent::Click {
            pos: [90.0, 29.0],
            button: MouseButton::Left,
        });
        assert_eq!(options.bits(), 0b10_0000);
    }

    #[test]
    fn an_options_grid_steps_by_row_and_toggles_under_the_cursor() {
        const CELLS: &[&str] = &["NW", "OP", "SV", "PR", "CL", "UN", "RE"];

        let options = Options::new(CELLS, Rect::new(0.0, 0.0, 300.0, 18.0));
        let mut host = ControlHost::builder()
            .with_named_control("wear", options.handle())
            .build();
        host.set_focused_index(Some(0));

        let key = |key: Key| KeyboardEvent::KeyDown { key };
        for (press, at) in [
            (Key::Right, 1usize),
            (Key::Down, 6),
            (Key::Up, 1),
            (Key::Right, 2),
            (Key::Left, 1),
        ] {
            assert!(host.deliver_keyboard(key(press)).handled);
            assert_eq!(options.cursor(), at, "{press:?} must step to {at}");
            assert_eq!(options.bits(), 0, "stepping lights nothing");
        }

        options.set_cursor(0);
        for press in [Key::Left, Key::Up] {
            assert_eq!(host.deliver_keyboard(key(press)).action, None);
            assert_eq!(options.cursor(), 0, "{press:?} must stop at the start");
        }
        options.set_cursor(CELLS.len() - 1);
        for press in [Key::Right, Key::Down] {
            assert_eq!(host.deliver_keyboard(key(press)).action, None);
            assert_eq!(
                options.cursor(),
                CELLS.len() - 1,
                "{press:?} must stop at the end",
            );
        }

        for bits in [1u32 << (CELLS.len() - 1), 0] {
            assert!(host.deliver_keyboard(key(Key::Enter)).handled);
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

        let mut host = ControlHost::builder()
            .with_named_control("tabs", radio.handle())
            .build();

        let pressed = host.deliver_mouse(MouseEvent::Click {
            pos: [90.0, 29.0],
            button: MouseButton::Left,
        });
        assert!(pressed.handled);
        assert_eq!(pressed.target_id.as_deref(), Some("tabs"));
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
        let mut host = ControlHost::builder().with_control(radio.handle()).build();

        let gap = host.deliver_mouse(MouseEvent::Click {
            pos: [41.5, 29.0],
            button: MouseButton::Left,
        });
        assert!(!gap.handled, "the gap between two cells is not a cell");

        assert!(radio.bounds().contains([250.0, 29.0]));
        let blank = host.deliver_mouse(MouseEvent::Click {
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
        let mut host = ControlHost::builder()
            .with_control(radio.handle())
            .with_initial_focus(0)
            .build();

        let stop_left = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
        assert!(
            stop_left.action.is_none(),
            "the first cell has nothing to its left"
        );
        assert_eq!(radio.selected_index(), 0);

        for expected in 1..=2 {
            let step = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
            assert!(step.handled);
            assert_eq!(step.action, Some(ControlAction::StepRight));
            assert_eq!(radio.selected_index(), expected);
        }

        let stop_right = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
        assert!(
            stop_right.action.is_none(),
            "the last cell has nothing to its right"
        );
        assert_eq!(radio.selected_index(), 2);

        let back = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Left });
        assert_eq!(back.action, Some(ControlAction::StepLeft));
        assert_eq!(radio.selected_index(), 1);

        let space = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Space });
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

        let mut host = ControlHost::builder().with_control(radio.handle()).build();
        host.deliver_mouse(MouseEvent::Click {
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
    fn control_host_builder_and_mouse_event_delivery() {
        let label = Label::new("Settings", Rect::new(10.0, 10.0, 300.0, 40.0));
        let sel = SelectableLabel::from_static(
            &["Option A", "Option B", "Option C"],
            Rect::new(10.0, 60.0, 300.0, 36.0),
        )
        .with_selected_index(1);
        let slider = Slider::new(50.0, 0.0, 100.0, Rect::new(10.0, 110.0, 300.0, 24.0));
        let num_slider = SliderNumeric::new(1.20, 0.5, 2.0, Rect::new(10.0, 150.0, 300.0, 32.0))
            .with_label("Scale");

        let mut host = ControlHost::builder()
            .with_named_control("title", label.handle())
            .with_named_control("mode_select", sel.handle())
            .with_named_control("volume_slider", slider.handle())
            .with_named_control("scale_slider", num_slider.handle())
            .build();

        assert_eq!(host.control_count(), 4);
        assert_eq!(host.find_by_id("volume_slider"), Some(2));

        let resp_left = host.deliver_mouse(MouseEvent::Click {
            pos: [15.0, 70.0],
            button: MouseButton::Left,
        });
        assert!(resp_left.handled);
        assert_eq!(resp_left.target_id.as_deref(), Some("mode_select"));
        assert_eq!(resp_left.target_part, Some(ControlPart::LeftArrow));
        assert_eq!(resp_left.action, Some(ControlAction::StepLeft));
        assert_eq!(host.focused_index(), Some(1));

        let resp_slider = host.deliver_mouse(MouseEvent::Click {
            pos: [235.0, 120.0],
            button: MouseButton::Left,
        });
        assert!(resp_slider.handled);
        assert_eq!(resp_slider.target_id.as_deref(), Some("volume_slider"));
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
        assert_eq!(host.focused_index(), Some(2));

        let resp_outside = host.deliver_mouse(MouseEvent::Click {
            pos: [400.0, 400.0],
            button: MouseButton::Left,
        });
        assert!(!resp_outside.handled);
        assert_eq!(host.focused_index(), None);
    }

    #[test]
    fn a_drag_keeps_hold_of_its_slider_across_a_rebuild() {
        let top = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let below = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut host = ControlHost::builder()
            .with_control(top.handle())
            .with_control(below.handle())
            .build();

        let down = host.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target_index, Some(0), "the press lands on the top one");
        let grab = host.active_drag().expect("the press must grab it");

        let mut next = ControlHost::builder()
            .with_control(top.handle())
            .with_control(below.handle())
            .build();
        next.set_active_drag(Some(grab));
        assert!(next.active_drag().is_some(), "the grab must be put back");

        let moved = next.deliver_mouse(MouseEvent::Move { pos: [180.0, 38.0] });
        assert_eq!(
            moved.target_index,
            Some(0),
            "the drag keeps the slider it grabbed, not the one under the pointer",
        );
        assert!(top.value() > 0.8, "and the one it grabbed is what moved");
        assert!(
            (below.value() - 0.5).abs() < 1e-6,
            "the one it wandered over must not have moved at all",
        );

        next.deliver_mouse(MouseEvent::Up {
            pos: [180.0, 38.0],
            button: MouseButton::Left,
        });
        assert!(next.active_drag().is_none(), "the release must let go");
    }

    #[test]
    fn sweeping_across_sliders_with_nothing_grabbed_moves_none_of_them() {
        let top = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let below = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));
        let mut host = ControlHost::builder()
            .with_control(top.handle())
            .with_control(below.handle())
            .build();

        host.deliver_mouse(MouseEvent::Move { pos: [190.0, 10.0] });
        host.deliver_mouse(MouseEvent::Move { pos: [190.0, 38.0] });
        assert_eq!(
            top.value(),
            0.5,
            "a move with nothing grabbed moves nothing"
        );
        assert_eq!(below.value(), 0.5, "and nothing means nothing");
    }

    #[test]
    fn a_cleared_focus_stays_cleared_across_a_rebuild() {
        let one = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let two = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));
        let panel = Panel::new(Rect::new(0.0, 0.0, 200.0, 60.0))
            .with_control(one.handle())
            .with_control(two.handle())
            .with_focus(None);
        assert!(
            !panel
                .deliver_event(InputEvent::Keyboard(KeyboardEvent::KeyDown {
                    key: Key::Space
                }))
                .handled,
            "with nothing focused a key press has nowhere to go",
        );
    }

    #[test]
    fn escape_reports_that_it_cleared_the_focus() {
        let only = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let mut host = ControlHost::builder()
            .with_control(only.handle())
            .with_initial_focus(0)
            .build();
        let escaped = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Escape });
        assert!(escaped.handled, "the clear must be reported");
        assert_eq!(escaped.focused_index, None);
        assert_eq!(host.focused_index(), None);
    }

    #[test]
    fn a_drag_grab_naming_a_control_that_is_gone_is_dropped() {
        let only = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let mut host = ControlHost::builder().with_control(only.handle()).build();
        host.set_active_drag(Some(DragGrab {
            key: ControlKey::at(7),
            start_pos: [0.0, 0.0],
            button: MouseButton::Left,
        }));
        assert!(host.active_drag().is_none(), "a grab on nothing is no grab");
    }

    #[test]
    fn a_focus_key_whose_control_is_gone_does_not_land_on_its_successor() {
        let level = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let leg = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let tab = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut open = ControlHost::builder()
            .with_named_control("tab", tab.handle())
            .with_named_control("level_slider", level.handle())
            .build();
        open.set_focused_index(Some(1));
        let carried = open.focused_key().expect("the focus must be carried");

        let mut next = ControlHost::builder()
            .with_named_control("tab", tab.handle())
            .with_named_control("leg_slider", leg.handle())
            .build();
        next.set_focused_key(Some(carried));
        assert_eq!(
            next.focused_index(),
            None,
            "the control the focus named is gone, so the focus is gone with it",
        );

        next.set_focused_key(Some(ControlKey::at(1)));
        assert_eq!(
            next.focused_index(),
            Some(1),
            "a place is a place, and something else is standing in it",
        );
    }

    #[test]
    fn a_drag_grab_whose_control_is_gone_lets_go() {
        let level = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let leg = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let tab = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));

        let mut open = ControlHost::builder()
            .with_named_control("tab", tab.handle())
            .with_named_control("level_slider", level.handle())
            .build();
        let down = open.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 10.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target_id.as_deref(), Some("level_slider"));
        let grab = open.active_drag().expect("the press must grab it");

        let mut next = ControlHost::builder()
            .with_named_control("tab", tab.handle())
            .with_named_control("leg_slider", leg.handle())
            .build();
        next.set_active_drag(Some(grab));
        assert!(
            next.active_drag().is_none(),
            "the control it had hold of is gone, so it is holding nothing",
        );

        next.deliver_mouse(MouseEvent::Move { pos: [190.0, 10.0] });
        assert_eq!(
            leg.value(),
            0.5,
            "and the slider standing where it was must not have moved",
        );

        next.set_active_drag(Some(DragGrab {
            key: ControlKey::at(1),
            start_pos: [20.0, 10.0],
            button: MouseButton::Left,
        }));
        next.deliver_mouse(MouseEvent::Move { pos: [190.0, 10.0] });
        assert!(leg.value() > 0.8, "which is what a place does");
    }

    #[test]
    fn a_control_that_has_moved_is_still_found_by_name() {
        let alpha = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 0.0, 200.0, 20.0));
        let beta = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 30.0, 200.0, 20.0));
        let inserted = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 60.0, 200.0, 20.0));

        let mut before = ControlHost::builder()
            .with_named_control("alpha", alpha.handle())
            .with_named_control("beta", beta.handle())
            .build();
        before.set_focused_index(Some(1));
        let down = before.deliver_mouse(MouseEvent::Down {
            pos: [20.0, 38.0],
            button: MouseButton::Left,
        });
        assert_eq!(down.target_id.as_deref(), Some("beta"));
        let focus = before.focused_key().expect("beta has the focus");
        let grab = before.active_drag().expect("and the press has hold of it");

        let mut after = ControlHost::builder()
            .with_named_control("alpha", alpha.handle())
            .with_named_control("inserted", inserted.handle())
            .with_named_control("beta", beta.handle())
            .build();
        after.set_focused_key(Some(focus));
        assert_eq!(
            after.focused_index(),
            Some(2),
            "beta is still here, further along",
        );

        after.set_active_drag(Some(grab));
        let moved = after.deliver_mouse(MouseEvent::Move { pos: [190.0, 38.0] });
        assert_eq!(moved.target_id.as_deref(), Some("beta"));
        assert!(beta.value() > 0.8, "and the drag still has hold of it");
        assert_eq!(
            inserted.value(),
            0.5,
            "not of whatever now sits where it used to",
        );
    }

    #[test]
    fn control_host_keyboard_event_delivery_and_focus_cycling() {
        let label = Label::new("Header", Rect::new(0.0, 0.0, 200.0, 30.0));
        let sel = SelectableLabel::from_static(
            &["Option 1", "Option 2"],
            Rect::new(0.0, 40.0, 200.0, 30.0),
        );
        let slider = Slider::new(0.5, 0.0, 1.0, Rect::new(0.0, 80.0, 200.0, 30.0));

        let mut host = ControlHost::builder()
            .with_control(label.handle())
            .with_control(sel.handle())
            .with_control(slider.handle())
            .with_initial_focus(0)
            .build();

        assert_eq!(host.focused_index(), Some(0));

        let tab_resp = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Tab });
        assert!(tab_resp.handled);
        assert_eq!(host.focused_index(), Some(1));

        let right_resp = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
        assert!(right_resp.handled);
        assert_eq!(right_resp.action, Some(ControlAction::StepRight));

        host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Tab });
        assert_eq!(host.focused_index(), Some(2));

        let slider_step = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::Right });
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

        let backtab_resp = host.deliver_keyboard(KeyboardEvent::KeyDown { key: Key::BackTab });
        assert!(backtab_resp.handled);
        assert_eq!(host.focused_index(), Some(1));
    }

    #[test]
    fn control_host_mutates_underlying_control_states_directly() {
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

        let mut host = ControlHost::builder()
            .with_control(sel.handle())
            .with_control(slider.handle())
            .with_control(btn.handle())
            .build();

        host.deliver_mouse(MouseEvent::Click {
            pos: [190.0, 15.0],
            button: MouseButton::Left,
        });
        assert_eq!(mode_state.selected_index(), 1);
        assert_eq!(sel.selected_index(), 1);

        host.deliver_mouse(MouseEvent::Down {
            pos: [160.0, 55.0],
            button: MouseButton::Left,
        });
        assert!((slider_state.value() - 0.8).abs() < 0.01);
        assert!((slider.value() - 0.8).abs() < 0.01);

        host.deliver_mouse(MouseEvent::Click {
            pos: [100.0, 95.0],
            button: MouseButton::Left,
        });
        assert!(btn_state.is_pressed());
        assert!(btn.is_pressed());
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

        let mut host = ControlHost::builder()
            .with_named_control("sn", sn.handle())
            .build();

        let (_, slider_r, _) = sn.sub_rects();
        let click_x = slider_r.x + slider_r.w * 0.5;
        let click_y = slider_r.y + slider_r.h * 0.5;
        host.deliver_mouse(MouseEvent::Click {
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
        let panel = Panel::new(Rect::new(0.0, 0.0, 200.0, 100.0))
            .with_named_control("toggle", button.handle());

        let response = panel.deliver_event(InputEvent::Mouse(MouseEvent::Click {
            pos: [40.0, 30.0],
            button: MouseButton::Left,
        }));

        assert!(response.handled);
        assert_eq!(response.target_id.as_deref(), Some("toggle"));
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
        let panel = Panel::new(Rect::new(0.0, 0.0, 200.0, 100.0))
            .with_title("PROPERTIES")
            .with_data(Arc::clone(&properties))
            .with_named_control("toggle", button.handle());

        let response = panel.deliver_event(InputEvent::Mouse(MouseEvent::Click {
            pos: [40.0, 30.0],
            button: MouseButton::Left,
        }));
        assert!(response.handled);

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
        let radio = Radio::new(CELLS, Rect::new(0.0, 0.0, 300.0, 18.0));
        assert_eq!(radio.shown_label(0), "SM");
        assert_eq!(radio.tooltip(0), Some("Small"));
        assert_eq!(radio.selected_label(), "SM - Small");
    }

    #[test]
    fn a_hovered_cell_is_the_one_under_the_pointer_and_nothing_else() {
        const CELLS: &[&str] = &["SM - Small", "MD - Medium", "LG - Large"];
        let radio = Radio::new(CELLS, Rect::new(10.0, 20.0, 300.0, 18.0));
        let mut host = ControlHost::builder()
            .with_named_control("cycle", radio.handle())
            .build();

        assert_eq!(radio.hovered_cell(), None, "nothing is hovered to start");

        let moved = host.deliver_mouse(MouseEvent::Move { pos: [90.0, 29.0] });
        assert!(!moved.handled, "a hover is not an interaction");
        assert_eq!(radio.hovered_cell(), Some(2));
        assert_eq!(radio.selected_index(), 0, "and it selects nothing");

        host.deliver_mouse(MouseEvent::Move { pos: [41.5, 29.0] });
        assert_eq!(radio.hovered_cell(), None);

        host.deliver_mouse(MouseEvent::Move { pos: [90.0, 29.0] });
        assert_eq!(radio.hovered_cell(), Some(2));

        host.deliver_mouse(MouseEvent::Move { pos: [90.0, 300.0] });
        assert_eq!(radio.hovered_cell(), None);
    }

    #[test]
    fn hovering_one_control_clears_the_hover_on_another() {
        const CELLS: &[&str] = &["SV - Save", "PR - Print"];
        let grid = Options::new(CELLS, Rect::new(0.0, 0.0, 200.0, 20.0));
        let button = Button::new("CT - Center", Rect::new(0.0, 40.0, 60.0, 20.0));
        let mut host = ControlHost::builder()
            .with_control(grid.handle())
            .with_control(button.handle())
            .build();

        host.deliver_mouse(MouseEvent::Move { pos: [15.0, 10.0] });
        assert_eq!(grid.hovered_cell(), Some(0));
        assert!(!button.is_hovered());
        assert_eq!(grid.tooltip(0), Some("Save"));

        host.deliver_mouse(MouseEvent::Move { pos: [30.0, 50.0] });
        assert!(button.is_hovered(), "the button has it now");
        assert_eq!(grid.hovered_cell(), None, "and the grid has let go");
        assert_eq!(button.shown_text(), "CT");
        assert_eq!(button.tooltip(), Some("Center"));

        host.deliver_mouse(MouseEvent::Down {
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
            .with_tooltip_style(TooltipStyle::default().with_bg(TIP_BG));
        let button = Button::new("CT", Rect::new(0.0, 40.0, 60.0, 20.0)).with_bg(BUTTON_BG);
        let mut host = ControlHost::builder()
            .with_control(grid.handle())
            .with_control(button.handle())
            .build();

        let ctx = DrawContext::default();
        let mut quiet = DrawList::new();
        host.draw(&mut quiet, &ctx);
        assert!(
            !quiet.instances.iter().any(|inst| inst.color == TIP_BG),
            "nothing hovered draws no tip",
        );

        host.deliver_mouse(MouseEvent::Move { pos: [15.0, 10.0] });
        let mut list = DrawList::new();
        host.draw(&mut list, &ctx);

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
    fn abbreviation_is_optional_on_the_multi_cell_controls() {
        const NAMED: &[&str] = &["NW - New", "OP - Open", "XY"];
        let rect = Rect::new(0.0, 0.0, 300.0, 18.0);

        let radio = Radio::new(NAMED, rect);
        assert_eq!(radio.shown_label(0), "NW");
        assert_eq!(radio.tooltip(0), Some("New"));
        let options = Options::new(NAMED, rect);
        assert_eq!(options.shown_label(1), "OP");
        assert_eq!(options.tooltip(1), Some("Open"));

        let radio = Radio::new(NAMED, rect).with_abbreviate(false);
        assert_eq!(radio.shown_label(0), "New");
        assert_eq!(radio.tooltip(0), None);
        let options = Options::new(NAMED, rect).with_abbreviate(false);
        assert_eq!(options.shown_label(1), "Open");
        assert_eq!(options.tooltip(1), None);

        assert_eq!(radio.shown_label(2), "XY");
        assert_eq!(options.shown_label(2), "XY");
    }
}
