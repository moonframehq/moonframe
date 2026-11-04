//! What a panic leaves behind: a report on disk, written as it happens, and
//! `app.crash` at the next launch so a mod can offer to send or show it.
//! Nothing leaves the machine by itself.
//!
//! A panic a provider's thread survives is written too: the hook cannot tell.
//! A crash below Rust (a bad pointer in AppKit, say) never reaches it; macOS
//! keeps its own report of those in `~/Library/Logs/DiagnosticReports`.

use {
    moonframe_core::Data,
    std::{
        backtrace::Backtrace,
        collections::BTreeMap,
        fs,
        panic::PanicHookInfo,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Reports written in one run: a thread panicking in a loop must not fill
/// the disk.
const MOST_PER_RUN: usize = 5;
/// Reports kept across runs.
const KEPT: usize = 20;
/// What a report's name ends with until the app has told of it.
const NEW: &str = ".txt";
const TOLD: &str = ".told";

/// Writes a report to `dir` for each panic, then panics as before.
pub(crate) fn install(dir: PathBuf, app: String) {
    static WRITTEN: AtomicUsize = AtomicUsize::new(0);
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if WRITTEN.fetch_add(1, Ordering::Relaxed) < MOST_PER_RUN {
            let text = report(&app, info);
            // Nothing can be done about it in a panic.
            let _ = write(&dir, &text);
        }
        before(info);
    }));
}

fn report(app: &str, info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with no message".to_owned());
    let thread = std::thread::current();
    format!(
        "{message}\n\n{app}\n{} {}\nthread: {}\nat: {}\n\n{}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        thread.name().unwrap_or("unnamed"),
        info.location()
            .map_or_else(|| "unknown".to_owned(), ToString::to_string),
        Backtrace::force_capture(),
    )
}

fn write(dir: &Path, text: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let name = format!("panic-{}-{}{NEW}", now(), std::process::id());
    fs::write(dir.join(name), text)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

/// The newest report not told of yet, as `app.crash`, marking them told
/// and leaving the last [`KEPT`].
pub(crate) fn last(dir: &Path) -> Option<Data> {
    let mut reports: Vec<(SystemTime, PathBuf)> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let time = entry.metadata().and_then(|meta| meta.modified()).ok()?;
            Some((time, entry.path()))
        })
        .collect();
    reports.sort();
    let is_new = |path: &Path| path.to_string_lossy().ends_with(NEW);
    let newest = reports
        .iter()
        .rev()
        .find(|(_, path)| is_new(path))
        .map(|(_, path)| path.clone());
    let data = newest.as_deref().and_then(told_of);
    let surplus = reports.len().saturating_sub(KEPT);
    for (index, (_, path)) in reports.iter().enumerate() {
        if index < surplus {
            let _ = fs::remove_file(path);
        } else if is_new(path) {
            let _ = fs::rename(path, path.with_extension(&TOLD[1..]));
        }
    }
    data
}

fn told_of(path: &Path) -> Option<Data> {
    let text = fs::read_to_string(path).ok()?;
    let told = path.with_extension(&TOLD[1..]);
    Some(Data::Map(BTreeMap::from([
        (
            "message".to_owned(),
            Data::from(text.lines().next().unwrap_or_default()),
        ),
        (
            "report".to_owned(),
            Data::from(told.to_string_lossy().as_ref()),
        ),
    ])))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn field(data: &Data, key: &str) -> Data {
        let Data::Map(map) = data else {
            panic!("not a map");
        };
        map[key].clone()
    }

    #[test]
    fn a_report_is_told_of_once_and_can_still_be_read() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "it broke\n\nmoonframe 1.0\n").unwrap();
        let data = last(dir.path()).unwrap();
        assert_eq!(field(&data, "message"), Data::from("it broke"));
        let Data::String(report) = field(&data, "report") else {
            panic!("no path");
        };
        assert!(
            fs::read_to_string(report)
                .unwrap()
                .contains("moonframe 1.0")
        );
        assert_eq!(last(dir.path()), None);
    }

    #[test]
    fn no_folder_is_no_crash() {
        assert_eq!(last(Path::new("/nonexistent/moonframe-crashes")), None);
    }

    #[test]
    fn only_the_last_reports_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..KEPT + 5 {
            fs::write(dir.path().join(format!("panic-{index}{TOLD}")), "old").unwrap();
        }
        last(dir.path());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), KEPT);
    }
}
