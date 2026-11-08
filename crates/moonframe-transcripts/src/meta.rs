//! A session's title, folder and branch, from its transcript.

use {
    crate::text::{contains, decode_object, find, line_at, one_line, quoted, rfind, typed},
    serde_json::Value,
    std::{fs, path::Path},
};

/// Where the folder and branch are looked for: the first lines say them.
const HEAD: usize = 256 * 1024;
/// Where the first thing typed is looked for, when there is no title.
const FIRST: usize = 4 << 20;
/// Lines longer than this are not the first thing typed.
const FIRST_LINE: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Meta {
    pub(crate) title: Value,
    pub(crate) cwd: Option<String>,
    pub(crate) branch: Option<String>,
    /// Whether the session opened a pull request.
    pub(crate) pr: bool,
}

/// Transcripts run to tens of megabytes and decoding JSON is the slow part,
/// so this reads the file whole and searches it, decoding only the few lines
/// it needs. `None` when it can't be read or has no title.
pub(crate) fn meta_of(path: &Path) -> Option<Meta> {
    let data = fs::read(path).ok()?;
    let head = &data[..data.len().min(HEAD)];
    let cwd = quoted(head, br#""cwd":""#);
    let branch = quoted(head, br#""gitBranch":""#);
    let pr = contains(&data, b"gh pr create");
    let field = |at: usize, key: &str| {
        decode_object(line_at(&data, at))
            .and_then(|entry| entry.get(key).cloned())
            .filter(|value| !value.is_null())
    };
    // The latest title the CLI gave it wins.
    let ai = rfind(&data, br#"{"type":"ai-title""#)
        .filter(|&at| at == 0 || data[at - 1] == b'\n')
        .and_then(|at| field(at, "aiTitle"));
    let title = ai
        .or_else(|| find(&data, br#"{"type":"summary""#, 0).and_then(|at| field(at, "summary")))
        .or_else(|| first_typed(&data[..data.len().min(FIRST)]))?;
    Some(Meta {
        title: one_line(&title),
        cwd,
        branch,
        pr,
    })
}

/// The first thing typed.
fn first_typed(data: &[u8]) -> Option<Value> {
    data.split(|&byte| byte == b'\n')
        .filter(|line| {
            line.len() < FIRST_LINE
                && contains(line, br#""type":"user""#)
                && !contains(line, br#""tool_result""#)
        })
        .filter_map(decode_object)
        .find_map(|entry| typed(&entry))
}
