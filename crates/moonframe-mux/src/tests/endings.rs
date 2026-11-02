//! A disconnected control client can reattach; a shell exit is final.

use super::*;

struct Server(String);

impl Server {
    fn command(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-L", &self.0, "-f", "/dev/null"])
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.command(&["kill-server"]);
    }
}

fn ended(session: &Session) -> Screen {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = session.screen();
        if screen.exited || screen.disconnected {
            return screen;
        }
        assert!(Instant::now() < deadline, "the stream never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn tmux_detach_reconnect_and_shell_exit_have_distinct_outcomes() {
    let Some(tmux) = Tmux::find() else {
        eprintln!("tmux is not installed; skipped");
        return;
    };
    // Both a server with another session and one that stops at the last exit.
    for keep_server in [false, true] {
        let server = Server(format!(
            "moonframe-end-test-{}-{keep_server}",
            std::process::id()
        ));
        if keep_server {
            assert!(
                server
                    .command(&["new-session", "-d", "-s", "keeper"])
                    .status
                    .success()
            );
        }
        let tmux = tmux.clone().on_socket(&server.0);
        let open = Open {
            key: "test/end".into(),
            cwd: None,
            size: Size::default(),
        };
        let start = || {
            Session::start(
                tmux.open(&open).unwrap(),
                Engine::Alacritty,
                open.size,
                Arc::new(|| {}),
            )
            .unwrap()
        };
        let first = start();
        first.type_when_ready(b"echo still-$((20 + 2))\r".to_vec());
        assert!(wait_for(&first, "still-22"));
        assert!(
            server
                .command(&["detach-client", "-s", "test+end"])
                .status
                .success()
        );
        let screen = ended(&first);
        assert!(screen.disconnected);
        assert!(!first.exited());
        assert!(
            server
                .command(&["has-session", "-t", "=test+end"])
                .status
                .success()
        );
        drop(first);

        // Enter uses the same Open/key, recovering the original shell and output.
        let second = start();
        assert!(wait_for(&second, "still-22"));
        second.write(b"echo reattached-$((2 + 2))\r");
        assert!(wait_for(&second, "reattached-4"));
        second.write(b"exit\r");
        let screen = ended(&second);
        assert!(second.exited(), "keep_server={keep_server}: {screen:?}");
        assert!(!screen.disconnected);
    }
}
