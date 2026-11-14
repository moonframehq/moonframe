//! How long a real agent takes to change an app, part by part. Ignored:
//! it needs an agent installed and signed in, and spends its tokens.
//!
//! ```sh
//! BENCH_REPO=/tmp/repo BENCH_APP=/tmp/repo/app \
//! BENCH_CHECK='target/release/moonframe --no-user --check /tmp/repo/app' \
//! BENCH_PROMPT='put a red background' \
//! cargo test -p moonframe-acp bench -- --ignored --nocapture
//! ```
//!
//! `BENCH_INSTRUCTIONS` names a file of the mod's instructions;
//! `BENCH_BRIEF=0` leaves out the brief (`app = true`); `BENCH_ALLOW` is
//! the feedback mod's level (`app` by default); `BENCH_WARM` warms the
//! agent that many seconds ahead; `BENCH_AGENT` picks the agent.
//! It prints the run as one line of JSON.

use super::*;

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[test]
#[ignore = "starts a real agent on a real app, which needs it installed and signed in"]
fn bench() {
    let (Some(repo), Some(app), Some(check), Some(prompt)) = (
        var("BENCH_REPO"),
        var("BENCH_APP"),
        var("BENCH_CHECK"),
        var("BENCH_PROMPT"),
    ) else {
        panic!("set BENCH_REPO, BENCH_APP, BENCH_CHECK and BENCH_PROMPT");
    };
    let store = tempfile::tempdir().unwrap();
    let allow = var("BENCH_ALLOW").unwrap_or_else(|| "app".to_owned());
    let settings = json!({
        "starters": ["feedback"],
        "cwd": repo,
        "store": store.path().join("runs.json"),
        "allow": { "feedback": allow },
    });
    let host = Handle::new(
        "acp",
        Path::new(&app),
        from_json(&settings).unwrap(),
        Arc::new(|_| {}),
    );
    let mut acp = Acp::default();
    acp.start(&host).unwrap();
    acp.editable = Some(Editable {
        title: "Bench".to_owned(),
        layers: vec![("app".to_owned(), PathBuf::from(&app))],
        app: PathBuf::from(&app),
        writable: PathBuf::from(&app),
        workdir: PathBuf::from(&repo),
        check,
        guide: None,
        state: None,
    });
    acp.writable = Some(PathBuf::from(&app));
    let agent = var("BENCH_AGENT").unwrap_or_else(|| "claude".to_owned());
    if let Some(seconds) = var("BENCH_WARM").and_then(|text| text.parse::<f64>().ok()) {
        let warm = json!({ "warm": true, "agent": agent });
        acp.receive(&host, "feedback", from_json(&warm).unwrap());
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }
    let instructions = var("BENCH_INSTRUCTIONS")
        .map(|file| std::fs::read_to_string(file).unwrap())
        .unwrap_or_default();
    let start = json!({
        "start": prompt,
        "agent": agent,
        "app": var("BENCH_BRIEF").as_deref() != Some("0"),
        "instructions": instructions,
    });
    let began = Instant::now();
    acp.receive(&host, "feedback", from_json(&start).unwrap());
    let deadline = began + Duration::from_secs(900);
    loop {
        acp.tick(&host);
        let run = acp.last.as_ref().unwrap()["runs"][0].clone();
        let status = run["status"].as_str().unwrap_or("");
        if status == "waiting" {
            panic!("asked: {}", run["question"]);
        }
        if !matches!(status, "starting" | "running") {
            let tools: Vec<Value> = run["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| part["tools"].as_array())
                .flatten()
                .map(|tool| json!([tool["kind"], tool["title"], tool["ms"]]))
                .collect();
            println!(
                "{}",
                json!({
                    "seconds": began.elapsed().as_secs_f64(),
                    "status": status,
                    "error": run["error"],
                    "tool_count": run["tool_count"],
                    "model": run["model"],
                    "timing": run["timing"],
                    "tools": tools,
                    "message": run["message"],
                })
            );
            return;
        }
        assert!(Instant::now() < deadline, "still {run}");
        std::thread::sleep(Duration::from_millis(100));
    }
}
