//! Reads this machine's plan usage once, as the `usage` provider would,
//! and prints what it publishes as JSON when the read is done: for seeing
//! what `app.usage` holds without starting the app. Keychain items no one
//! allowed are not read, so macOS asks nothing.

use {
    moonframe_core::Data,
    moonframe_providers::{Event, Handle, Native as _},
    std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
};

fn main() {
    let last = Arc::new(Mutex::new(None));
    let host = {
        let last = Arc::clone(&last);
        Handle::new(
            "usage",
            std::path::Path::new("."),
            Data::Nil,
            Arc::new(move |event| match event {
                Event::Published { data, .. } => {
                    *last.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(moonframe_providers::to_json(&data));
                },
                Event::Failed { problem, .. } => eprintln!("{problem}"),
                _ => {},
            }),
        )
    };
    let mut usage = moonframe_usage::UsageProvider::default();
    if let Err(problem) = usage.start(&host) {
        eprintln!("{problem}");
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut started = false;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(250));
        usage.tick(&host);
        let busy = last
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|data| data["busy"] == true);
        started |= busy;
        if started && !busy {
            break;
        }
    }
    let data = last.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(data) = data {
        println!("{data:#}");
    }
}
