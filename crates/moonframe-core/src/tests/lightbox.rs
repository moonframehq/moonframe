//! Restoring the real scanner console must restore its process-local holds.
use {
    super::*,
    crate::{Awake, Guard, Request},
};

fn reading(running: bool) -> Data {
    Data::Map(
        [
            ("running".into(), Data::Bool(running)),
            ("done".into(), Data::Bool(false)),
            ("frame".into(), Data::Integer(10)),
            ("total".into(), Data::Integer(720)),
        ]
        .into(),
    )
}

fn holds(requests: &[(String, Request)]) {
    assert!(requests.iter().any(|(owner, request)| owner == "console"
        && matches!(request, Request::Awake(Awake::Hold { display: false, .. }))));
    assert!(requests.iter().any(|(owner, request)| owner == "console"
        && matches!(request, Request::Guard(Guard::Hold { .. }))));
}

#[test]
fn a_restored_lightbox_scan_reacquires_and_releases_its_holds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = crate::Roots::bundled(
        root.join("app"),
        vec![],
        crate::AppLayer {
            path: root.join("examples/lightbox"),
            keep: vec![],
        },
    );
    let memory = Memory::default();
    let mut workspace = Workspace::open_with(roots.clone(), false, Some(Box::new(memory.clone())));
    assert!(workspace.take_requests().is_empty());
    workspace.set_host("scanner", reading(true));
    holds(&workspace.take_requests());
    workspace.send("console", "console", Data::from("parade"));
    workspace.send("console", "console", Data::from("stop-and-close"));
    workspace.flush();
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
    drop(workspace);
    // A close prompt saved by the previous process is transient too.
    if let Some(Data::Map(state)) = memory.0.borrow_mut().0.get_mut("mod:console") {
        state.insert("closing".into(), Data::from("quit"));
    }

    let mut workspace = Workspace::open_with(roots, false, Some(Box::new(memory.clone())));
    let mut requests = workspace.take_requests();
    workspace.set_host("scanner", reading(true));
    requests.extend(workspace.take_requests());
    holds(&requests);
    assert!(
        requests
            .iter()
            .any(|(_, request)| matches!(request, Request::Dock(_)))
    );
    workspace.set_host("scanner", reading(true));
    assert!(
        workspace.take_requests().is_empty(),
        "repeated readings must not renew holds"
    );
    let saved = memory.0.borrow().0["mod:console"].clone();
    let Data::Map(fields) = saved else {
        panic!("not a map")
    };
    assert_eq!(fields.get("parade"), Some(&Data::Bool(false)));
    assert_eq!(fields.get("leaving"), Some(&Data::Bool(false)));
    assert!(!fields.contains_key("closing"));

    workspace.set_host("scanner", reading(false));
    let requests = workspace.take_requests();
    assert!(
        requests
            .iter()
            .any(|(_, request)| matches!(request, Request::Awake(Awake::Release)))
    );
    assert!(
        requests
            .iter()
            .any(|(_, request)| matches!(request, Request::Guard(Guard::Stop)))
    );
    assert!(
        !requests
            .iter()
            .any(|(_, request)| matches!(request, Request::Guard(Guard::Release)))
    );
    assert!(
        workspace.problems().is_empty(),
        "{:?}",
        workspace.problems()
    );
}
