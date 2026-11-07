use super::*;

const MB: u64 = 1024 * 1024;

fn peak() -> Peak {
    Peak::new(64 * MB, MB)
}

#[test]
fn a_small_state_says_nothing() {
    let peak = peak();
    assert_eq!(peak.read(MB / 2), Report::Quiet);
    assert_eq!(peak.seen.get(), MB / 2);
}

#[test]
fn only_real_growth_is_a_line() {
    let peak = peak();
    assert_eq!(peak.read(2 * MB), Report::Grew);
    assert_eq!(peak.read(2 * MB + MB / 4), Report::Quiet);
    assert_eq!(peak.read(3 * MB), Report::Grew);
}

#[test]
fn falling_keeps_the_peak_and_says_nothing() {
    let peak = peak();
    peak.read(8 * MB);
    assert_eq!(peak.read(2 * MB), Report::Quiet);
    assert_eq!(peak.seen.get(), 8 * MB);
}

#[test]
fn nearing_the_limit_warns_once() {
    let peak = peak();
    assert_eq!(peak.read(48 * MB), Report::Near);
    assert_eq!(peak.read(49 * MB), Report::Quiet);
    assert_eq!(peak.read(62 * MB), Report::Grew);
}

#[test]
fn a_real_state_is_read_at_the_end_of_each_call() {
    let meter = crate::vm::Meter::default();
    let library = crate::component::Library::default();
    let lua = crate::vm::open("test", &meter, &library, crate::vm::MEMORY).unwrap();
    let before = lua.used_memory();
    lua.load("local t = {} for i = 1, 100000 do t[i] = i end return t")
        .eval::<mlua::Table>()
        .unwrap();
    assert!(lua.used_memory() > before + 1024 * 1024);
    assert!(meter.used() > 0);
}
