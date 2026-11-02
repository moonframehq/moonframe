//! Drive the real example through the host's input, topic and view paths.
use super::*;

fn data(source: &str) -> Data {
    let lua = mlua::Lua::new();
    Data::read(&lua.load(source).eval().unwrap()).unwrap()
}

fn field<'a>(value: &'a Data, key: &str) -> &'a Data {
    let Data::Map(fields) = value else {
        panic!("not a map: {value:?}")
    };
    fields.get(key).unwrap_or(&Data::Nil)
}

fn composer(node: &Node) -> Option<&crate::Input> {
    match node {
        Node::Input(input) if input.id.as_deref() == Some("composer") => Some(input),
        Node::Stack(stack) => stack.children.iter().find_map(composer),
        Node::Popover(popover) => popover.anchor.iter().find_map(composer),
        _ => None,
    }
}

struct App {
    workspace: Workspace,
    memory: Memory,
    _profile: tempfile::TempDir,
}

impl App {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profile = tempfile::tempdir().unwrap();
        let roots = crate::Roots::profile(
            root.join("app"),
            [root.join("examples/projects")],
            Some(crate::AppLayer {
                path: root.join("examples/switchyard"),
                keep: vec!["feedback".into()],
            }),
            None,
            profile.path().to_owned(),
        );
        let memory = Memory::default();
        let workspace = Workspace::open_with(roots, false, Some(Box::new(memory.clone())));
        let mut app = Self {
            workspace,
            memory,
            _profile: profile,
        };
        app.select("/work");
        app.workspace.take_outbox();
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

    fn send(&mut self, module: &str, source: &str) {
        self.workspace.send(module, module, data(source));
        self.check();
    }

    fn select(&mut self, path: &str) {
        self.send(
            "workspaces",
            &format!("return {{select={{path={path:?},name='repo',branch='main'}}}}"),
        );
    }

    fn state(&self, module: &str) -> Data {
        self.memory.0.borrow().0[&format!("mod:{module}")].clone()
    }

    fn outbox(&mut self, provider: &str, key: &str) -> Vec<Data> {
        self.workspace
            .take_outbox()
            .into_iter()
            .filter(|(to, _, msg)| to == provider && field(msg, key) != &Data::Nil)
            .map(|(_, _, msg)| msg)
            .collect()
    }

    fn start(&mut self) {
        self.send("chat", "return {pick_new='test'}");
        let request = self.outbox("chat", "start").remove(0);
        let reply = Data::Map(
            [
                ("reply".into(), field(&request, "id").clone()),
                ("action".into(), Data::from("start")),
                ("ok".into(), Data::Bool(true)),
                ("chat".into(), Data::from("one")),
            ]
            .into(),
        );
        self.workspace.send("provider:chat", "chat", reply);
        self.workspace.take_outbox();
    }

    fn live(&mut self, entries: &str) {
        self.workspace.set_host("chat", data(&format!(
            "return {{chats={{{{id='one',agent='test',cwd='/work',status='live',transcript={{status={{state='idle'}},entries={entries}}}}}}}}}"
        )));
        self.check();
    }
}

#[test]
fn composer_keys_queue_until_attached_and_recover_rejected_prompts() {
    let mut app = App::new();
    app.start();
    // These are the actual input-key messages, including the current text.
    let enter = crate::Input::message(&Data::from("send"), "first");
    app.workspace.send("chat", "chat", enter);
    app.send("chat", "return {msg='send',value='second'}");
    assert!(app.outbox("chat", "prompt").is_empty());
    app.live("{}");
    let prompts = app.outbox("chat", "prompt");
    assert_eq!(
        prompts
            .iter()
            .map(|p| field(p, "prompt"))
            .collect::<Vec<_>>(),
        [&Data::from("first"), &Data::from("second")]
    );
    app.live("{}");
    assert!(
        app.outbox("chat", "prompt").is_empty(),
        "sent a queued prompt twice"
    );
    let rejection = Data::Map(
        [
            ("reply".into(), field(&prompts[0], "id").clone()),
            ("action".into(), Data::from("prompt")),
            ("ok".into(), Data::Bool(false)),
        ]
        .into(),
    );
    app.workspace.send("provider:chat", "chat", rejection);
    assert_eq!(field(&app.state("chat"), "draft"), &Data::from("first"));
    app.send("chat", "return {msg='newline',value='fresh input'}");
    assert_eq!(
        field(&app.state("chat"), "draft"),
        &Data::from("fresh input\n")
    );
}

#[test]
fn shell_enter_and_cards_use_the_selected_workspaces_anchor() {
    let mut app = App::new();
    app.start();
    app.live("{{id=100,kind={type='agent',text='old'}}}");
    app.select("/other");
    app.send("chat", "return {msg='send',value='!pwd'}");
    let request = app.outbox("shell", "run").remove(0);
    assert_eq!(field(&request, "run"), &Data::from("pwd"));
    assert_eq!(field(&request, "cwd"), &Data::from("/other"));
    let state = app.state("chat");
    let Data::String(id) = field(&request, "id") else {
        panic!()
    };
    assert_eq!(
        field(field(field(&state, "pending"), id), "after"),
        &Data::Integer(0)
    );
    app.select("/work");
    app.send("chat", "return {msg='send',value='!pwd'}");
    let request = app.outbox("shell", "run").remove(0);
    let Data::String(id) = field(&request, "id") else {
        panic!()
    };
    assert_eq!(
        field(field(field(&app.state("chat"), "pending"), id), "after"),
        &Data::Integer(100)
    );
}

#[test]
fn long_conversations_and_expanded_reasoning_stay_within_host_limits() {
    let mut app = App::new();
    app.start();
    let entries = "(function() local entries={} for i=1,20 do table.insert(entries,{id=i,kind={type='agent',text=string.rep('word ',600)}}) end table.insert(entries,{id=21,kind={type='thought',text=string.rep('reason ',8000)}}) return entries end)()";
    app.live(entries);
    app.send("chat", "return {toggle='one:21'}");
    app.check();
}

#[test]
fn completion_requests_nested_directories_and_preserves_quoted_words() {
    let mut app = App::new();
    app.send("chat", "return {msg='draft',value='!cat src/ma'}");
    let request = app.outbox("projects", "complete").remove(0);
    assert_eq!(field(&request, "complete"), &Data::from("src"));
    assert_eq!(field(&request, "cwd"), &Data::from("/work"));
    app.send("chat", "return {msg='draft',value='!cat src/main'}");
    assert!(
        app.outbox("projects", "complete").is_empty(),
        "listing requested on every keystroke"
    );
    // A quoted directory completion remains one word on the next Tab.
    app.send("chat", r#"return {msg='draft',value="!cat 'My Dir/'ma"}"#);
    assert_eq!(
        field(&app.outbox("projects", "complete")[0], "complete"),
        &Data::from("My Dir")
    );
    app.send("chat", r#"return {complete="'My Dir/main file.rs'"}"#);
    assert_eq!(
        field(&app.state("chat"), "draft"),
        &Data::from("!cat 'My Dir/main file.rs'")
    );
}

#[cfg(unix)]
#[test]
fn generated_completions_are_literal_shell_arguments() {
    let mut app = App::new();
    for name in [
        "My File.txt",
        "My'File.txt",
        "My$(echo surprise).txt",
        "My Dir/",
    ] {
        let entry = if name.ends_with('/') {
            format!("d{}", name.trim_end_matches('/'))
        } else {
            format!("f{name}")
        };
        app.workspace.set_host("projects", data(&format!(
            "return {{current={{path='/work',name='repo',branch='main',files={{}},count=0}},projects={{}},dirs={{['']={{{entry:?}}}}}}}"
        )));
        app.send("chat", "return {msg='draft',value='!cat My'}");
        let input = composer(app.workspace.slot("chat").unwrap().view().unwrap()).unwrap();
        let (_, completion) = input.keys.iter().find(|(key, _)| key.key == "tab").unwrap();
        let completion = completion.clone();
        let Data::String(insert) = field(&completion, "complete") else {
            panic!()
        };
        let output = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("set -- {insert}; printf '%s\\n' \"$#\" \"$1\""))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("1\n{name}\n")
        );
        app.workspace.send(
            "chat",
            "chat",
            crate::Input::message(&completion, "!cat My"),
        );
        app.check();
        assert_eq!(
            field(&app.state("chat"), "draft"),
            &Data::from(format!("!cat {insert}"))
        );
    }
}

#[test]
fn checks_restore_the_subscription_and_follow_branch_changes() {
    let mut app = App::new();
    app.send("inspector", "return {tab='checks'}");
    app.workspace.take_outbox();
    app.workspace.reload();
    assert!(
        app.outbox("projects", "forge")
            .iter()
            .any(|msg| field(msg, "forge") == &Data::Bool(true))
    );
    app.workspace.set_host("projects", data("return {current={path='/work',name='repo',branch='feature',files={},count=0},projects={}}"));
    assert_eq!(
        field(&app.state("inspector"), "branch"),
        &Data::from("feature")
    );
    app.check();
}
