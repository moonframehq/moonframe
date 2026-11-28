//! Live diagrams from agents.
//!
//! - [`mcp`]: `moonframe mcp diagrams`, an MCP server over stdio whose
//!   tools (`diagram_show`, `diagram_patch`, `diagram_highlight`,
//!   `diagram_close`) let an agent show the user diagrams and keep them up
//!   to date. Moonframe offers it to the agents it starts; any other agent
//!   can add the same command to its MCP settings.
//! - [`provider`]: the `diagrams` native provider, which shares those and
//!   a project's `.moonframe/diagrams` files as `app.diagrams`.
//!
//! The two meet in a folder ([`store::agents_dir`]), one JSON file per
//! diagram, so neither waits for the other.

pub mod document;
pub mod mcp;
pub mod provider;
pub mod store;

pub use provider::native;

/// Serves the MCP server on stdin and stdout until stdin ends, writing
/// diagrams to `dir` (else the default folder) on behalf of `agent`, for
/// the agent run `run` when the app started it.
pub fn serve_stdio(
    dir: Option<std::path::PathBuf>,
    agent: Option<String>,
    run: Option<String>,
) -> std::io::Result<()> {
    let dir = dir.unwrap_or_else(store::default_dir);
    let stdin = std::io::stdin();
    mcp::Server::new(&dir, agent)
        .for_run(run)
        .serve(stdin.lock(), std::io::stdout().lock())
}
