//! `moonframe mcp <server>`: the MCP servers this build serves over stdio,
//! for the agents it starts (offered with each session) and for any agent
//! told the command. Handled before anything else starts, as stdout is the
//! protocol's.
//!
//!   moonframe mcp diagrams [--dir FOLDER] [--agent NAME]

#[cfg(feature = "diagram")]
use std::path::PathBuf;
use std::process::ExitCode;

/// `Some` when the command line asks for an MCP server: its exit code.
pub(crate) fn run_if_asked() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.strip_prefix(&["mcp".to_owned()])?;
    Some(run(rest))
}

fn run(words: &[String]) -> ExitCode {
    let mut words = words.iter();
    match words.next().map(String::as_str) {
        #[cfg(feature = "diagram")]
        Some("diagrams") => {
            let (mut dir, mut agent) = (None, None);
            while let Some(word) = words.next() {
                match word.as_str() {
                    "--dir" => dir = words.next().map(PathBuf::from),
                    "--agent" => agent = words.next().cloned(),
                    other => {
                        eprintln!("moonframe mcp diagrams: unknown option `{other}`");
                        return ExitCode::from(2);
                    },
                }
            }
            match moonframe_diagrams::serve_stdio(dir, agent) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("moonframe mcp diagrams: {e}");
                    ExitCode::FAILURE
                },
            }
        },
        other => {
            eprintln!(
                "moonframe mcp: no server `{}` in this build{}",
                other.unwrap_or(""),
                if cfg!(feature = "diagram") {
                    "; it has `diagrams`"
                } else {
                    ""
                }
            );
            ExitCode::from(2)
        },
    }
}

/// The MCP servers to offer each agent session: this executable serving
/// `diagrams`, as ACP's `mcpServers` lists a stdio server.
#[cfg(all(feature = "diagram", feature = "acp"))]
pub(crate) fn offer() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let dir = moonframe_diagrams::store::default_dir();
    moonframe_acp::offer_mcp_servers(vec![serde_json::json!({
        "name": "moonframe-diagrams",
        "command": exe,
        "args": ["mcp", "diagrams", "--dir", dir],
        "env": [],
    })]);
}
