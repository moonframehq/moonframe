//! Fills a folder with real threads to look at: the real pi against the mock
//! model, through the provider, for a window started with
//! `PI_CODING_AGENT_DIR=DIR/agent PILOT_DATA=DIR/data`.
//!
//! ```sh
//! cargo run -p moonframe-pi --example pilot_seed -- DIR [PORT] [--stay]
//! ```
//!
//! `--stay` goes on serving the mock model on PORT afterwards, so that
//! window can run more threads.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "../tests/support/mod.rs"]
mod support;

use {
    serde_json::json,
    std::{env, fs, path::PathBuf, thread, time::Duration},
    support::{App, config, fixture_project, items, mock_agent_dir, mock_llm, real_pi},
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = PathBuf::from(args.first().expect("usage: pilot_seed DIR [PORT] [--stay]"));
    let port = args.get(1).and_then(|port| port.parse().ok()).unwrap_or(0);
    let stay = args.iter().any(|arg| arg == "--stay");
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let port = mock_llm::serve(port).unwrap();
    let agent = mock_agent_dir(&root, port);
    let project = fixture_project(&root);
    let settings = config(&root, &agent, real_pi());
    let mut app = App::start("pi", moonframe_pi::pi_with(settings));
    app.when("models", |v| {
        v["models"].as_array().is_some_and(|list| !list.is_empty())
    });
    app.send(&json!({ "add_workspace": project }));
    app.send(&json!({ "set_model": { "provider": "mock", "id": "mock-1" } }));
    app.send(&json!({ "set_pref": { "auto_title": true } }));
    app.when("workspace", |v| {
        v["workspaces"]
            .as_array()
            .is_some_and(|list| !list.is_empty())
    });
    for prompt in [
        "Summarise this repo and suggest improvements.",
        "Review the cartTotal edge cases",
    ] {
        app.send(
            &json!({ "new_thread": { "workspace": project, "mode": "local", "prompt": prompt } }),
        );
        app.when("answered", |v| {
            v["thread"]["title"] != "New thread"
                && !items(v, "text").is_empty()
                && v["thread"]["status"] == "idle"
        });
        thread::sleep(Duration::from_millis(800));
    }
    let fix = "formatPrice in src/price.js prints $12.5 instead of $12.50. Fix it, add a test for \
               cents padding, and run the tests.";
    app.send(&json!({ "new_thread": { "workspace": project, "mode": "local", "prompt": fix } }));
    app.when("fixed", |v| {
        !items(v, "changes").is_empty() && v["thread"]["status"] == "idle"
    });
    thread::sleep(Duration::from_millis(1500));
    app.stop();
    println!(
        "seeded {}; the mock model is on port {port}",
        root.display()
    );
    if stay {
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }
}
