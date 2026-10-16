//! Interned control names.

use libcontrols::{Button, ControlHost, ControlKey, MouseButton, MouseEvent, Rect, Symbol, sym};

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
    let a = Button::new("A", Rect::new(0.0, 0.0, 50.0, 20.0));
    let b = Button::new("B", Rect::new(60.0, 0.0, 50.0, 20.0));
    let host = ControlHost::builder()
        .with_named_control("first", a.handle())
        .with_named_control(sym!("second"), b.handle())
        .build();
    assert_eq!(
        host.find_by_id("second"),
        Some(1),
        "a str name must still work"
    );
    assert_eq!(
        host.find_by_id(sym!("first")),
        Some(0),
        "and so must a symbol"
    );
    assert_eq!(host.find_by_id("missing"), None);
}

#[test]
fn the_response_names_the_control_as_a_symbol() {
    let speed = Button::new("Speed", Rect::new(0.0, 0.0, 50.0, 20.0));
    let mut host = ControlHost::builder()
        .with_named_control("speed", speed.handle())
        .build();
    let resp = host.deliver_mouse(MouseEvent::Down {
        pos: [10.0, 10.0],
        button: MouseButton::Left,
    });
    assert_eq!(
        resp.target_id.as_deref(),
        Some("speed"),
        "as_deref still reads a str"
    );
    assert_eq!(
        resp.target_id,
        Some(sym!("speed")),
        "and the id is the symbol"
    );
}

#[test]
fn a_control_key_is_copy_and_names_its_symbol() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<ControlKey>();
    let key = ControlKey::named("level");
    let copy = key;
    assert_eq!(key, copy);
    assert_eq!(key.id(), Some(Symbol::intern("level")));
    assert_eq!(ControlKey::at(3).id(), None, "a slot has no name");
}
