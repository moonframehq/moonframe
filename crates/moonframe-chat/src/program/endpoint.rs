//! Discover only sockets owned by the process we started. A newly created
//! socket in the shared endpoint directory may belong to another launch.

use {
    herdr_chat_protocol::EndpointId,
    std::{
        path::Path,
        process::{Command, Stdio},
    },
};

pub(super) fn listening(pid: u32, folder: &Path) -> Option<EndpointId> {
    #[cfg(target_os = "linux")]
    if let Some(endpoint) = proc_endpoint(pid, folder) {
        return Some(endpoint);
    }
    let output = Command::new("lsof")
        .args(["-a", "-U", "-Fn", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .find_map(|path| endpoint_of(Path::new(path), folder))
}

fn endpoint_of(path: &Path, folder: &Path) -> Option<EndpointId> {
    if path.parent() != Some(folder) || path.extension()? != "sock" {
        return None;
    }
    path.file_stem()?.to_str()?.parse().ok()
}

#[cfg(target_os = "linux")]
fn proc_endpoint(pid: u32, folder: &Path) -> Option<EndpointId> {
    let inodes: std::collections::HashSet<String> = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .ok()?
        .flatten()
        .filter_map(|entry| std::fs::read_link(entry.path()).ok())
        .filter_map(|target| {
            target
                .to_str()?
                .strip_prefix("socket:[")?
                .strip_suffix(']')
                .map(str::to_owned)
        })
        .collect();
    let sockets = std::fs::read_to_string(format!("/proc/{pid}/net/unix")).ok()?;
    sockets.lines().skip(1).find_map(|line| {
        // The path may contain spaces; the first seven fields never do.
        let mut rest = line;
        let mut inode = "";
        for _ in 0..7 {
            rest = rest.trim_start();
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            inode = &rest[..end];
            rest = &rest[end..];
        }
        inodes
            .contains(inode)
            .then(|| endpoint_of(Path::new(rest.trim_start()), folder))
            .flatten()
    })
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn another_process_socket_is_not_our_endpoint() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let endpoint = EndpointId::random();
        let address = dir.path().join(format!("{endpoint}.sock"));
        let _listener = UnixListener::bind(&address).unwrap();
        let mut other = Command::new("/bin/sleep").arg("10").spawn().unwrap();
        #[cfg(target_os = "linux")]
        wait_for_exec(other.id());
        let found = listening(other.id(), dir.path());
        other.kill().unwrap();
        other.wait().unwrap();
        assert!(found.is_none(), "claimed another process's socket");
        assert_eq!(listening(std::process::id(), dir.path()), Some(endpoint));
    }

    /// Waits for `pid` to run `sleep`: until it execs, a forked child still
    /// holds this process's descriptors, the test's socket among them, and
    /// `/proc` shows it as the child's. Exec closes it (close-on-exec).
    #[cfg(target_os = "linux")]
    fn wait_for_exec(pid: u32) {
        for _ in 0..500 {
            let exe = std::fs::read_link(format!("/proc/{pid}/exe"));
            if exe.is_ok_and(|exe| exe.ends_with("sleep")) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{pid} never ran sleep");
    }
}
