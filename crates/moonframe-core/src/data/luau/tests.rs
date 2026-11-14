use {super::STRING, crate::Data};

#[test]
fn values_read_as_luau() {
    let state = Data::map([
        ("open", Data::Bool(true)),
        ("count", Data::Integer(3)),
        ("my key", Data::from("a \"b\"")),
        ("rows", Data::from(vec![Data::from("x"), Data::Integer(2)])),
        ("none", Data::Map(Default::default())),
    ]);
    assert_eq!(
        state.to_luau(1000),
        "{\n  count = 3,\n  [\"my key\"] = \"a \\\"b\\\"\",\n  none = {},\n  open = true,\n  rows = {\n    \"x\",\n    2,\n  },\n}"
    );
}

#[test]
fn big_values_are_cut_and_say_so() {
    let long = "x".repeat(STRING + 7);
    assert!(
        Data::from(long.as_str())
            .to_luau(1000)
            .ends_with("--[[ 7 more chars ]]")
    );

    let many = Data::from((0..500).map(Data::Integer).collect::<Vec<_>>());
    let text = many.to_luau(200);
    assert!(text.len() < 400, "{text}");
    assert!(text.contains("more"), "{text}");
    assert!(text.ends_with("-- cut to fit"), "{text}");

    let mut deep = Data::Integer(1);
    for _ in 0..20 {
        deep = Data::map([("in", deep)]);
    }
    assert!(deep.to_luau(10_000).contains("--[[ 1 entries ]]"));
}

#[test]
fn a_line_stays_on_one_line() {
    let msg = Data::map([("open", Data::from("settings")), ("at", Data::Integer(2))]);
    assert_eq!(msg.to_luau_line(100), "{ at = 2, open = \"settings\", }");
    let many = Data::from((0..500).map(Data::Integer).collect::<Vec<_>>());
    let text = many.to_luau_line(100);
    assert!(!text.contains('\n'), "{text}");
    assert!(text.ends_with("--[[ cut to fit ]]"), "{text}");
}

#[test]
fn keys_and_strings_stay_valid_luau() {
    let tricky = Data::map([
        ("end", Data::Bool(true)),
        ("bell", Data::from("a\u{7}b\tc")),
        (&*"k".repeat(STRING + 50), Data::Integer(1)),
    ]);
    let text = tricky.to_luau_line(10_000);
    assert!(text.contains("[\"end\"] = true,"), "{text}");
    assert!(text.contains("\"a\\u{7}b\\tc\""), "{text}");
    // A long key is quoted and cut like any string.
    assert!(
        text.contains(&format!(
            "[\"{}\" --[[ 50 more chars ]]]",
            "k".repeat(STRING)
        )),
        "{text}"
    );
}

#[test]
fn numbers_read_back_as_luau() {
    let line = |value: f64| Data::Number(value).to_luau_line(100);
    assert_eq!(line(f64::NAN), "0/0");
    assert_eq!(line(f64::INFINITY), "math.huge");
    assert_eq!(line(f64::NEG_INFINITY), "-math.huge");
    assert_eq!(line(1e300), "1e300");
    assert_eq!(line(-2.5e-9), "-2.5e-9");
    assert_eq!(line(0.25), "0.25");
    assert_eq!(line(1500.), "1500");
}
