use {
    crate::{MIGRATIONS, Store},
    moonframe_core::Data,
    rusqlite::Connection,
};

fn sample() -> Data {
    Data::Map(
        [
            ("count".to_owned(), Data::Integer(3)),
            ("ratio".to_owned(), Data::Number(0.5)),
            (
                "open".to_owned(),
                Data::List(vec![Data::from("app"), Data::from("crates")]),
            ),
            ("on".to_owned(), Data::Bool(true)),
        ]
        .into(),
    )
}

#[test]
fn state_saves_and_reads_back() {
    let store = Store::in_memory().unwrap();
    assert_eq!(store.state("mod:explorer"), None);
    store.save_state("mod:explorer", &Data::from("old"));
    store.save_state("mod:explorer", &sample());
    store.flush();
    assert_eq!(store.state("mod:explorer"), Some(sample()));
    store.forget_state("mod:explorer");
    store.flush();
    assert_eq!(store.state("mod:explorer"), None);
}

#[test]
fn topics_save_and_list() {
    let store = Store::in_memory().unwrap();
    store.save_topic("tabs/main", &sample());
    store.save_topic("theme", &Data::from("dark"));
    store.flush();
    let topics = store.topics();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics["tabs/main"], sample());
}

#[test]
fn a_file_is_migrated_once_and_kept_across_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/state.sqlite");
    {
        let store = Store::open(&path).unwrap();
        assert!(store.writable());
        store.save_state("mod:shell", &sample());
        store.flush();
    }
    let version: i64 = Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(usize::try_from(version).unwrap(), MIGRATIONS.len());
    // Opening again runs nothing and finds what was saved.
    let store = Store::open(&path).unwrap();
    assert_eq!(store.state("mod:shell"), Some(sample()));
}

#[test]
fn an_older_file_gets_the_steps_it_lacks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite");
    // As the first release left it: only the first step.
    {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(MIGRATIONS[0]).unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection
            .execute(
                "INSERT INTO state (key, value, updated_at) VALUES ('mod:a', '\"kept\"', 0)",
                [],
            )
            .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.state("mod:a"), Some(Data::from("kept")));
    store.save_topic("t", &Data::Bool(true));
    store.flush();
    assert_eq!(store.topics()["t"], Data::Bool(true));
}

#[test]
fn a_file_from_a_newer_app_is_read_but_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite");
    {
        let store = Store::open(&path).unwrap();
        store.save_state("mod:a", &Data::from("theirs"));
        store.flush();
    }
    Connection::open(&path)
        .unwrap()
        .pragma_update(
            None,
            "user_version",
            i64::try_from(MIGRATIONS.len() + 1).unwrap(),
        )
        .unwrap();
    let store = Store::open(&path).unwrap();
    assert!(!store.writable());
    assert_eq!(store.state("mod:a"), Some(Data::from("theirs")));
    store.save_state("mod:a", &Data::from("ours"));
    store.flush();
    assert_eq!(store.state("mod:a"), Some(Data::from("theirs")));
}

#[test]
fn a_damaged_value_reads_as_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite");
    drop(Store::open(&path).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO state (key, value, updated_at) VALUES ('mod:a', '{not json', 0)",
            [],
        )
        .unwrap();
    let store = Store::open(&path).unwrap();
    assert_eq!(store.state("mod:a"), None);
}
