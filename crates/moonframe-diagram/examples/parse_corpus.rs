//! Parses every `.mmd`, `.mermaid` and `.dot` file in a folder, lays each
//! out, and reports what failed and why: to measure the parsers against
//! diagrams agents really wrote, kept outside the repository.
//!
//!   cargo run --release -p moonframe-diagram --example parse-corpus -- DIR

use {
    moonframe_diagram::{
        layout::{Approx, Options, layout},
        parse,
    },
    std::time::Instant,
};

fn main() {
    let Some(dir) = std::env::args().nth(1) else {
        eprintln!("usage: parse-corpus DIR");
        std::process::exit(2);
    };
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    let (mut ok, mut failed, mut slowest) = (0, 0, (0.0f64, String::new()));
    for path in paths {
        let language = match path.extension().and_then(|e| e.to_str()) {
            Some("mmd" | "mermaid") => "mermaid",
            Some("dot" | "gv") => "dot",
            _ => continue,
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match parse::fenced(language, &text) {
            Some(Ok(diagram)) => {
                let start = Instant::now();
                let _ = layout(&diagram, &Approx, &Options::default(), None);
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                if ms > slowest.0 {
                    slowest = (ms, name.clone());
                }
                ok += 1;
            },
            Some(Err(e)) => {
                failed += 1;
                println!("{name}: {e}");
            },
            None => {},
        }
    }
    let total = ok + failed;
    println!(
        "{ok} of {total} parsed ({:.0} %); slowest layout {:.2} ms ({})",
        ok as f64 * 100.0 / total.max(1) as f64,
        slowest.0,
        slowest.1
    );
}
