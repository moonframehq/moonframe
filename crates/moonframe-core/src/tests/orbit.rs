//! Exercise the shipped task-manager app through its real update/view path.
use super::*;

fn field<'a>(value: &'a Data, key: &str) -> &'a Data {
    let Data::Map(fields) = value else {
        panic!("not a map: {value:?}")
    };
    fields.get(key).unwrap_or(&Data::Nil)
}

struct Orbit {
    workspace: Workspace,
    memory: Memory,
    _profile: tempfile::TempDir,
}

impl Orbit {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profile = tempfile::tempdir().unwrap();
        let roots = crate::Roots::profile(
            root.join("app"),
            [],
            Some(crate::AppLayer {
                path: root.join("examples/orbit"),
                keep: vec![],
            }),
            None,
            profile.path().to_owned(),
        );
        let memory = Memory::default();
        let workspace = Workspace::open_with(roots, false, Some(Box::new(memory.clone())));
        let app = Self {
            workspace,
            memory,
            _profile: profile,
        };
        app.check();
        app
    }

    fn check(&self) {
        assert!(
            self.workspace.problems().is_empty(),
            "{:?}",
            self.workspace.problems()
        );
    }

    fn send(&mut self, source: &str) {
        let lua = mlua::Lua::new();
        let msg = Data::read(&lua.load(source).eval().unwrap()).unwrap();
        self.workspace.send("workspace", "workspace", msg);
        self.check();
    }

    fn state(&self) -> Data {
        self.memory.0.borrow().0["mod:workspace"].clone()
    }

    fn task(&self, id: &str) -> Data {
        let state = self.state();
        let Data::List(tasks) = field(&state, "tasks") else {
            panic!("no tasks")
        };
        tasks
            .iter()
            .find(|task| field(task, "id") == &Data::from(id))
            .unwrap()
            .clone()
    }
}

#[test]
fn orbit_views_and_task_edits_share_persisted_data() {
    let mut app = Orbit::new();
    app.send("return {select='WEB-104'}");
    for view in ["list", "calendar", "timeline", "board"] {
        app.send(&format!("return {{view={view:?}}}"));
    }
    for picker in ["status", "assignee", "priority", "date"] {
        app.send(&format!("return {{picker={picker:?}}}"));
        app.send("return 'close-picker'");
    }
    app.send("return {msg='title',value='Navigation, revised'}");
    app.send(r"return {msg='description',value=string.rep('Long description.\n\n',100)}");
    app.send("return {description_view='write'}");
    app.send("return {description_view='preview'}");
    app.send("return {status='review'}");
    app.send("return {priority='urgent'}");
    app.send("return {assignee='emma'}");
    app.send("return {due='2026-12-02'}");
    app.send("return {subtask=2}");
    let task = app.task("WEB-104");
    assert_eq!(field(&task, "title"), &Data::from("Navigation, revised"));
    assert_eq!(field(&task, "status"), &Data::from("review"));
    assert_eq!(field(&task, "assignee"), &Data::from("emma"));
    app.workspace.reload();
    app.check();
    assert_eq!(app.task("WEB-104"), task);
    for page in ["home", "mobile", "system", "mine", "website"] {
        app.send(&format!("return {{page={page:?}}}"));
    }
    app.send("return 'new-task'");
    let Data::String(id) = field(&app.state(), "selected").clone() else {
        panic!("not selected")
    };
    assert_eq!(field(&app.task(&id), "title"), &Data::from("Untitled task"));
}

#[test]
fn orbit_drags_reorder_and_reschedule_the_same_task() {
    let mut app = Orbit::new();
    app.send("return {card_drop='WEB-107',phase='over',y=0.1}");
    app.send("return {card_drop='WEB-107',phase='drop',y=0.1,carry={task='WEB-104'}}");
    assert_eq!(field(&app.task("WEB-104"), "status"), &Data::from("todo"));
    let state = app.state();
    let Data::List(tasks) = field(&state, "tasks") else {
        panic!()
    };
    let at = |id| {
        tasks
            .iter()
            .position(|t| field(t, "id") == &Data::from(id))
            .unwrap()
    };
    assert_eq!(at("WEB-104") + 1, at("WEB-107"));
    app.send("return {view='list'}");
    app.send("return {table_drop=true,group='done',phase='drop',carry={row='WEB-104'}}");
    assert_eq!(field(&app.task("WEB-104"), "status"), &Data::from("done"));
    app.send("return {view='calendar'}");
    app.send("return {calendar_view='week'}");
    app.send("return {calendar_view='day'}");
    app.send("return {calendar_drop='2026-11-12',phase='drop',y=0.2,carry={event='WEB-104'}}");
    let before = app.task("WEB-104");
    app.send("return {view='timeline'}");
    app.send("return {timeline_move='WEB-104',phase='move',value=3}");
    app.send("return {timeline_move='WEB-104',phase='up',value=3}");
    let after = app.task("WEB-104");
    for key in ["start", "finish"] {
        let Data::Integer(value) = field(&before, key) else {
            panic!()
        };
        assert_eq!(field(&after, key), &Data::Integer(value + 3));
    }
    app.send("return {timeline_resize='WEB-104',phase='up',value=-100}");
    let task = app.task("WEB-104");
    assert_eq!(field(&task, "start"), field(&task, "finish"));
}

#[test]
fn orbit_filters_comments_reactions_and_attachment_replies_work() {
    let mut app = Orbit::new();
    app.send("return 'filter'");
    app.send("return 'add'");
    app.send("return {open='c1-value'}");
    app.send("return {set={id='c1',key='value',value='review'}}");
    app.send("return {match='any'}");
    app.send("return 'done'");
    app.send("return {msg='search',value='no matching task'}");
    for view in ["list", "calendar", "timeline", "board"] {
        app.send(&format!("return {{view={view:?}}}"));
    }
    app.send("return {msg='search',value=''}");
    app.send("return 'clear'");
    app.send("return {select='WEB-104'}");
    app.send("return {msg='comment',value='Thanks @em',caret=10}");
    app.send("return 'close-mentions'");
    app.send("return 'post-comment'");
    app.send("return {reaction_picker=2}");
    app.send("return {msg='emoji-search',value='rocket'}");
    app.send("return {emoji='🚀'}");
    let task = app.task("WEB-104");
    let Data::List(comments) = field(&task, "comments") else {
        panic!()
    };
    assert_eq!(
        field(comments.last().unwrap(), "body"),
        &Data::from("Thanks @em")
    );
    let Data::List(reactions) = field(comments.last().unwrap(), "reactions") else {
        panic!()
    };
    assert_eq!(field(&reactions[0], "emoji"), &Data::from("🚀"));
    // A file chooser may answer after another task has been selected.
    app.send("return {select='WEB-105'}");
    app.send("return {attach='WEB-104',paths={'/example/brief.txt'}}");
    let task = app.task("WEB-104");
    let Data::List(files) = field(&task, "files") else {
        panic!()
    };
    assert_eq!(
        field(files.last().unwrap(), "path"),
        &Data::from("/example/brief.txt")
    );
}

#[test]
fn orbit_links_open_tasks_and_propose_new_ones() {
    let mut app = Orbit::new();
    let link = |path: &str, query: &[(&str, &str)]| {
        Data::Map(
            [
                ("path".to_owned(), Data::from(path)),
                (
                    "query".to_owned(),
                    Data::Map(
                        query
                            .iter()
                            .map(|(name, value)| ((*name).to_owned(), Data::from(*value)))
                            .collect(),
                    ),
                ),
            ]
            .into(),
        )
    };
    app.workspace
        .app_news("url", link("/task", &[("id", "APP-202")]));
    app.check();
    assert_eq!(field(&app.state(), "selected"), &Data::from("APP-202"));
    assert_eq!(field(&app.state(), "page"), &Data::from("mobile"));

    // A new task is only proposed until the user confirms it.
    let before = app.state();
    app.workspace
        .app_news("url", link("/new", &[("title", "Call the printer")]));
    app.check();
    assert_eq!(field(&app.state(), "tasks"), field(&before, "tasks"));
    assert_eq!(
        field(field(&app.state(), "offer"), "title"),
        &Data::from("Call the printer")
    );
    app.send("return 'drop-offer'");
    assert_eq!(field(&app.state(), "offer"), &Data::Nil);
    assert_eq!(field(&app.state(), "tasks"), field(&before, "tasks"));

    app.workspace
        .app_news("url", link("/new", &[("title", "Call the printer")]));
    app.check();
    app.send("return 'add-offer'");
    assert_eq!(field(&app.state(), "offer"), &Data::Nil);
    let Data::String(id) = field(&app.state(), "selected").clone() else {
        panic!("not selected")
    };
    assert_eq!(
        field(&app.task(&id), "title"),
        &Data::from("Call the printer")
    );
    assert_eq!(field(&app.task(&id), "project"), &Data::from("mobile"));

    // One it doesn't know changes nothing.
    app.workspace
        .app_news("url", link("/task", &[("id", "NOPE-1")]));
    app.check();
    assert_eq!(field(&app.state(), "selected"), &Data::String(id));
}
