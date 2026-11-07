use super::*;

/// Enough checks to tell apart from a module that did nothing.
const WORK: u64 = 1_000_000;

fn load(source: &str) -> Module {
    Module::load("test", source, None, None, &Library::default()).unwrap()
}

#[test]
fn the_work_of_init_is_read_before_the_next_call_starts_the_count_over() {
    let mut module = load(
        "return mod {
            init = function() for i = 1, 1000000 do end return {} end,
            update = function() end,
            view = function() return ui.text('') end,
        }",
    );
    assert!(module.peaks.seen().1 >= WORK);
    module.update(&Data::Nil).unwrap();
    assert!(module.peaks.seen().1 >= WORK);
}

#[test]
fn the_work_of_its_top_level_is_read_with_or_without_init() {
    for init in ["", "init = function() return {} end,"] {
        let module = load(&format!(
            "for i = 1, 1000000 do end
            return mod {{ {init} view = function() return ui.text('') end }}"
        ));
        assert!(module.peaks.seen().1 >= WORK, "with `{init}`");
    }
}

#[test]
fn the_memory_init_keeps_is_read() {
    let module = load(
        "return mod {
            init = function()
                local kept = {}
                for i = 1, 100000 do kept[i] = i end
                return { kept = kept }
            end,
            view = function() return ui.text('') end,
        }",
    );
    assert!(module.peaks.seen().0 > 1024 * 1024);
}
