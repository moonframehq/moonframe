use {super::*, std::path::PathBuf};

fn value(variables: &[(String, String)], name: &str) -> Option<String> {
    variables
        .iter()
        .find(|(variable, _)| variable == name)
        .map(|(_, value)| value.clone())
}

#[test]
fn zsh_is_led_through_our_folder_and_back_to_the_users() {
    let folder = PathBuf::from("/cache/shell");
    let plain = environment("/bin/zsh", &folder, |_| None);
    assert_eq!(value(&plain, "ZDOTDIR").unwrap(), "/cache/shell/zsh");
    assert_eq!(value(&plain, "MOONFRAME_ZDOTDIR"), None);

    let own = environment("/bin/zsh", &folder, |name| {
        (name == "ZDOTDIR").then(|| "/home/me/.config/zsh".to_owned())
    });
    assert_eq!(
        value(&own, "MOONFRAME_ZDOTDIR").unwrap(),
        "/home/me/.config/zsh"
    );
}

#[test]
fn fish_keeps_the_data_folders_it_had() {
    let folder = PathBuf::from("/cache/shell");
    let unset = environment("/opt/homebrew/bin/fish", &folder, |_| None);
    assert_eq!(
        value(&unset, "XDG_DATA_DIRS").unwrap(),
        "/cache/shell/fish:/usr/local/share:/usr/share"
    );
    let set = environment("fish", &folder, |name| {
        (name == "XDG_DATA_DIRS").then(|| "/nix/share".to_owned())
    });
    assert_eq!(
        value(&set, "XDG_DATA_DIRS").unwrap(),
        "/cache/shell/fish:/nix/share"
    );
}

#[test]
fn other_shells_only_hear_where_the_scripts_are() {
    let folder = PathBuf::from("/cache/shell");
    for shell in ["/bin/bash", "/bin/dash", ""] {
        let variables = environment(shell, &folder, |_| None);
        assert_eq!(
            variables,
            [(FOLDER.to_owned(), "/cache/shell/bash".to_owned())]
        );
    }
}
