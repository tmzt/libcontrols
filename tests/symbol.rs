//! Interned control names.

use libcontrols::{Button, MouseButton, MouseEvent, NodeId, Rect, Router, Symbol, sym};

#[test]
fn interning_the_same_name_gives_the_same_symbol() {
    let a = Symbol::intern("volume");
    let b = Symbol::intern(&String::from("volume"));
    assert_eq!(a, b, "one name must intern to one symbol");
    assert_eq!(a.id(), b.id());
}

#[test]
fn different_names_are_different_symbols() {
    assert_ne!(
        Symbol::intern("speed"),
        Symbol::intern("speed2"),
        "two names must not share a symbol"
    );
}

#[test]
fn a_symbol_reads_back_as_its_name() {
    let s = Symbol::intern("rotate");
    assert_eq!(s.as_str(), "rotate");
    assert_eq!(&*s, "rotate", "Deref must give the name");
    assert_eq!(s.to_string(), "rotate", "Display is the bare name");
    assert_eq!(format!("{s:?}"), "\"rotate\"", "Debug is the quoted name");
    assert_eq!(Some(s).as_deref(), Some("rotate"));
}

#[test]
fn a_symbol_compares_with_a_str() {
    let s = Symbol::intern("shake");
    assert!(s == "shake");
    assert!(s == *"shake");
    assert!(s != "save");
}

#[test]
fn sym_interns_once_per_call_site() {
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(sym!("zoom"));
    }
    assert!(
        seen.iter().all(|s| *s == seen[0]),
        "one call site, one symbol"
    );
    assert_eq!(seen[0], Symbol::intern("zoom"));
}

#[test]
fn interning_from_many_threads_agrees() {
    let names: Vec<String> = (0..20).map(|i| format!("threaded-{i}")).collect();
    let results: Vec<Vec<Symbol>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| names.iter().map(|n| Symbol::intern(n)).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for other in &results[1..] {
        assert_eq!(&results[0], other, "every thread must get the same symbols");
    }
}

#[test]
fn symbol_order_is_interning_order() {
    let first = Symbol::intern("order-first-only-here");
    let second = Symbol::intern("order-second-only-here");
    assert!(first < second, "a later name orders after an earlier one");
}

#[test]
fn a_symbol_is_a_plain_shareable_value() {
    fn assert<T: Copy + Eq + std::hash::Hash + Ord + Send + Sync>() {}
    assert::<Symbol>();
}

#[test]
fn a_named_control_is_found_by_str_and_by_symbol() {
    let mut router = Router::new();
    let first = router
        .add_named(
            None,
            "first",
            Button::new("A", Rect::new(0.0, 0.0, 50.0, 20.0)),
        )
        .unwrap();
    let second = router
        .add_named(
            None,
            sym!("second"),
            Button::new("B", Rect::new(60.0, 0.0, 50.0, 20.0)),
        )
        .unwrap();
    assert_eq!(router.find("second"), Some(second), "a str alias works");
    assert_eq!(
        router.find_alias(sym!("first")),
        Some(first),
        "and so does a symbol"
    );
    assert_eq!(router.find("missing"), None);
    assert_eq!(
        Symbol::lookup("never-interned-by-anyone"),
        None,
        "a failed find interns nothing"
    );
}

#[test]
fn the_response_names_the_control_by_id() {
    let mut router = Router::new();
    let speed = router
        .add_named(
            None,
            "speed",
            Button::new("Speed", Rect::new(0.0, 0.0, 50.0, 20.0)),
        )
        .unwrap();
    let resp = router.deliver_mouse(MouseEvent::Down {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    assert_eq!(resp.target, Some(speed), "the app compares the ids it kept");
    assert_eq!(router.find("speed"), resp.target);
}

#[test]
fn a_node_id_is_copy_and_names_itself() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<NodeId>();
    let mut router = Router::new();
    let id = router.add(None, Button::new("L", Rect::new(0.0, 0.0, 10.0, 10.0)));
    let copy = id;
    assert_eq!(id, copy);
    assert_eq!(copy.to_string().parse::<NodeId>(), Ok(id));
}
