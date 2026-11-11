use {
    super::{
        format::{Clock, DateLength, DateStyle, Formats},
        *,
    },
    std::fs,
};

const ENGLISH: &str = r#"
inbox-title = Inbox
inbox-unread = { $count ->
    [one] One unread message
   *[other] { $count } unread messages
}
only-english = Only in English
compose = Compose
    .tooltip = Write a new message
"#;

const FRENCH: &str = r#"
inbox-title = Boîte de réception
inbox-unread = { $count ->
    [one] Un message non lu
   *[other] { $count } messages non lus
}
compose = Écrire
    .tooltip = Écrire un nouveau message
"#;

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn language(tag: &str) -> LanguageIdentifier {
    tag.parse().unwrap()
}

/// An app with English and French messages, read for `requested`.
fn words(requested: &[&str]) -> (Words, Vec<Error>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "locales/en-US/inbox.ftl", ENGLISH);
    write(dir.path(), "locales/fr/inbox.ftl", FRENCH);
    let requested: Vec<_> = requested.iter().map(|tag| language(tag)).collect();
    let (words, problems) = Words::read(&Roots::new([dir.path().to_owned()]), &requested);
    (words, problems, dir)
}

fn count(count: i64) -> FluentArgs<'static> {
    let mut args = FluentArgs::new();
    args.set("count", count);
    args
}

#[test]
fn a_message_shows_in_the_language_the_user_prefers() {
    let (words, problems, _dir) = words(&["fr-CA", "en-US"]);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(words.locale(), "fr");
    assert_eq!(words.text("inbox-title", None), "Boîte de réception");
    assert_eq!(
        words.text("compose.tooltip", None),
        "Écrire un nouveau message"
    );
}

#[test]
fn what_a_translation_lacks_shows_in_english_then_as_its_id() {
    let (words, _, _dir) = words(&["fr"]);
    assert_eq!(words.text("only-english", None), "Only in English");
    assert_eq!(words.text("nowhere", None), "nowhere");
    assert_eq!(words.text("compose.nowhere", None), "compose.nowhere");
}

#[test]
fn a_language_the_app_lacks_falls_back_to_english() {
    let (words, _, _dir) = words(&["de-DE", "ja"]);
    assert_eq!(words.locale(), FALLBACK);
    assert_eq!(words.text("inbox-title", None), "Inbox");
}

#[test]
fn plurals_follow_the_language_and_numbers_its_digits() {
    let (french, _, _dir) = words(&["fr"]);
    assert_eq!(
        french.text("inbox-unread", Some(&count(1))),
        "Un message non lu"
    );
    // French groups thousands with a narrow no-break space.
    assert_eq!(
        french.text("inbox-unread", Some(&count(1234))),
        "1\u{202f}234 messages non lus"
    );
    let (english, _, _dir) = words(&["en-US"]);
    assert_eq!(
        english.text("inbox-unread", Some(&count(1))),
        "One unread message"
    );
    assert_eq!(
        english.text("inbox-unread", Some(&count(1234))),
        "1,234 unread messages"
    );
}

#[test]
fn a_higher_layer_replaces_a_message_and_keeps_the_rest() {
    let (_, _, app) = words(&["en-US"]);
    let user = tempfile::tempdir().unwrap();
    write(
        user.path(),
        "locales/en-US/mine.ftl",
        "inbox-title = Mail\n",
    );
    let roots = Roots::new([app.path().to_owned(), user.path().to_owned()]);
    let (words, problems) = Words::read(&roots, &[language("en-US")]);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(words.text("inbox-title", None), "Mail");
    assert_eq!(words.text("compose", None), "Compose");
}

#[test]
fn broken_files_and_stray_translations_are_problems() {
    let (_, _, dir) = words(&["fr"]);
    write(
        dir.path(),
        "locales/fr/extra.ftl",
        "typo-id = Coquille\n\nbroken = { $\n",
    );
    write(dir.path(), "locales/not a tag/x.ftl", "a = b\n");
    let (words, problems) = Words::read(&Roots::new([dir.path().to_owned()]), &[language("fr")]);
    let problems: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("extra.ftl: line 3")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("`typo-id` is not in locales/en-US")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("`not a tag` is not a language tag")),
        "{problems:?}"
    );
    // The rest still reads.
    assert_eq!(words.text("compose", None), "Écrire");
}

#[test]
fn numbers_are_written_the_languages_way() {
    let english = Formats::new(&language("en-US"));
    let french = Formats::new(&language("fr"));
    let german = Formats::new(&language("de"));
    assert_eq!(english.number(1234567.891, None), "1,234,567.891");
    assert_eq!(french.number(1234.5, None), "1\u{202f}234,5");
    assert_eq!(german.number(1234.5, Some(2)), "1.234,50");
    assert_eq!(english.number(0.1 + 0.2, None), "0.3");
    assert_eq!(english.number(-0.0001, None), "0");
    assert_eq!(english.number(2.0, Some(0)), "2");
    assert_eq!(english.number(-1500.0, None), "-1,500");
    assert_eq!(english.number(f64::NAN, None), "NaN");
}

#[test]
fn dates_are_written_the_languages_way() {
    // Noon UTC: the same day in every time zone a test runs in.
    let noon = 1_794_312_000.0;
    let style = |length| DateStyle {
        length,
        date: true,
        time: None,
    };
    let english = Formats::new(&language("en-US"));
    let french = Formats::new(&language("fr"));
    assert_eq!(
        english.date(noon, style(DateLength::Medium)).unwrap(),
        "Nov 10, 2026"
    );
    assert_eq!(
        english.date(noon, style(DateLength::Long)).unwrap(),
        "November 10, 2026"
    );
    assert_eq!(
        english.date(noon, style(DateLength::Short)).unwrap(),
        "11/10/26"
    );
    assert_eq!(
        french.date(noon, style(DateLength::Medium)).unwrap(),
        "10 nov. 2026"
    );
    let time = english
        .date(noon, DateStyle {
            length: DateLength::Short,
            date: false,
            time: Some(Clock::Seconds),
        })
        .unwrap();
    assert!(time.contains(":00"), "{time}");
    assert_eq!(english.date(f64::INFINITY, style(DateLength::Short)), None);
}

#[test]
fn system_tags_read_as_language_tags() {
    assert_eq!(posix_to_bcp47("pt_BR.UTF-8@euro"), "pt-BR");
    assert_eq!(posix_to_bcp47("fr-CA"), "fr-CA");
}
