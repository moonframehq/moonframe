//! Sounds and notifications mods ask for (`app:sound`, `app:notify`).
//!
//! - Sounds: a short file from the app's folders (checked by the caller)
//!   or a chime the app makes itself. macOS plays them with `NSSound`;
//!   Linux with `paplay` or `aplay` when one is installed; Windows plays
//!   none yet.
//! - Notifications: macOS posts them with `UNUserNotificationCenter`
//!   when the app runs as a bundle (the system refuses a bare binary);
//!   everywhere else, and from a bare binary, a toast in the window shows
//!   them instead. A click on one brings the window forward, and the mod
//!   that posted it hears it.
//!
//! Both are rate-limited, so a mod in a loop cannot flood the speakers or
//! the notification centre; so are web pages opened (`app:open`), so it
//! cannot flood the browser either.

#[cfg(target_os = "macos")]
mod macos;

use {
    moonframe_core::Notification,
    std::{
        collections::VecDeque,
        path::Path,
        time::{Duration, Instant},
    },
};

/// Sounds closer together than this are dropped.
const SOUND_GAP: Duration = Duration::from_millis(150);
/// At most this many notifications in [`NOTIFY_WINDOW`].
const NOTIFY_BURST: usize = 5;
const NOTIFY_WINDOW: Duration = Duration::from_secs(10);
/// At most this many web pages opened in [`NOTIFY_WINDOW`].
const OPEN_BURST: usize = 3;
/// Largest sound file played.
pub(crate) const MAX_SOUND_BYTES: u64 = 2 * 1024 * 1024;

/// Separates the parts of a notification's identifier, which carries
/// who posted it and its `id` through the system and back.
const SEPARATOR: char = '\u{1f}';

/// How a notification was shown.
#[derive(Debug, PartialEq)]
pub(crate) enum Posted {
    /// By the system (macOS, in a bundle).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    System,
    /// Not by the system: the app shows it as a toast.
    Toast,
}

#[derive(Default)]
pub(crate) struct Alerts {
    last_sound: Option<Instant>,
    posted: VecDeque<Instant>,
    opened: VecDeque<Instant>,
    sequence: u64,
    #[cfg(target_os = "macos")]
    sounds: macos::Sounds,
}

/// Keeps the times in `times` within [`NOTIFY_WINDOW`] and adds now,
/// unless `burst` of them are there already.
fn admit(times: &mut VecDeque<Instant>, burst: usize) -> bool {
    let now = Instant::now();
    while times
        .front()
        .is_some_and(|at| now.duration_since(*at) >= NOTIFY_WINDOW)
    {
        times.pop_front();
    }
    if times.len() >= burst {
        return false;
    }
    times.push_back(now);
    true
}

impl Alerts {
    /// Whether a web page may open now (`app:open`).
    pub(crate) fn may_open(&mut self) -> bool {
        admit(&mut self.opened, OPEN_BURST)
    }

    /// Plays `file`, or the chime when `None`. Too soon after the last one
    /// is an error, as is a file the platform cannot play.
    pub(crate) fn play(&mut self, file: Option<&Path>) -> Result<(), String> {
        let now = Instant::now();
        if self
            .last_sound
            .is_some_and(|last| now.duration_since(last) < SOUND_GAP)
        {
            return Err("sounds closer than 150 ms apart are dropped".to_owned());
        }
        self.last_sound = Some(now);
        self.play_now(file)
    }

    #[cfg(target_os = "macos")]
    fn play_now(&mut self, file: Option<&Path>) -> Result<(), String> {
        match file {
            Some(file) => self.sounds.play_file(file),
            None => self.sounds.play_bytes(&chime()),
        }
    }

    #[cfg(target_os = "linux")]
    fn play_now(&mut self, file: Option<&Path>) -> Result<(), String> {
        let path = match file {
            Some(file) => file.to_owned(),
            None => {
                // The app's own cache, the user's alone: never a fixed name
                // in the shared temporary folder, where another user could
                // leave a file or a link in its place.
                let path = crate::paths::cache().join("chime.wav");
                if !path.is_file() {
                    if let Some(folder) = path.parent() {
                        std::fs::create_dir_all(folder)
                            .map_err(|error| format!("{}: {error}", folder.display()))?;
                    }
                    std::fs::write(&path, chime())
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                }
                path
            },
        };
        // PulseAudio's (PipeWire's too), then ALSA's when the first is
        // missing or fails (no sound server), with no shell between. They
        // run to the end off the main thread, and what failed is logged.
        std::thread::spawn(move || {
            let mut problems = Vec::new();
            for program in ["paplay", "aplay"] {
                let output = std::process::Command::new(program)
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .output();
                match output {
                    Ok(output) if output.status.success() => return,
                    Ok(output) => problems.push(format!(
                        "{program}: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    )),
                    Err(error) => problems.push(format!("{program}: {error}")),
                }
            }
            tracing::warn!("no sound played: {}", problems.join("; "));
        });
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn play_now(&mut self, _: Option<&Path>) -> Result<(), String> {
        Err("sounds are not played on this platform yet".to_owned())
    }

    /// Posts `notification` for the mod `from`: by the system where it
    /// can, else it is for a toast. More than [`NOTIFY_BURST`] in
    /// [`NOTIFY_WINDOW`] is an error.
    pub(crate) fn notify(
        &mut self,
        from: &str,
        notification: &Notification,
    ) -> Result<Posted, String> {
        if !admit(&mut self.posted, NOTIFY_BURST) {
            return Err(format!(
                "more than {NOTIFY_BURST} notifications in {} s are dropped",
                NOTIFY_WINDOW.as_secs()
            ));
        }
        self.sequence += 1;
        let identifier = identifier(self.sequence, from, notification.id.as_deref());
        #[cfg(target_os = "macos")]
        if macos::post(&identifier, notification) {
            return Ok(Posted::System);
        }
        let _ = identifier;
        Ok(Posted::Toast)
    }
}

/// Has `clicked` called with the mod and the `id` of each notification the
/// user clicks, from now on: where the system posts them (macOS, in a
/// bundle). Call it once, at launch, so a click that opens the app is
/// heard.
pub(crate) fn listen(clicked: impl Fn(String, Option<String>) + Send + Sync + 'static) {
    #[cfg(target_os = "macos")]
    macos::listen(Box::new(move |identifier: &str| {
        if let Some((from, id)) = parse_identifier(identifier) {
            clicked(from, id);
        }
    }));
    #[cfg(not(target_os = "macos"))]
    let _ = clicked;
}

/// A notification's identifier: unique, and carrying who posted it and
/// its `id` back when it is clicked.
fn identifier(sequence: u64, from: &str, id: Option<&str>) -> String {
    format!(
        "moonframe{SEPARATOR}{sequence}{SEPARATOR}{from}{SEPARATOR}{}",
        id.unwrap_or("")
    )
}

/// Who posted the notification `identifier`, and its `id`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_identifier(identifier: &str) -> Option<(String, Option<String>)> {
    let mut parts = identifier.split(SEPARATOR);
    if parts.next()? != "moonframe" {
        return None;
    }
    let _sequence = parts.next()?;
    let from = parts.next().filter(|from| !from.is_empty())?;
    let id = parts.next()?;
    Some((from.to_owned(), (!id.is_empty()).then(|| id.to_owned())))
}

/// A short two-note chime as a WAV file: 16-bit mono at 44.1 kHz, made
/// here rather than shipped.
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), allow(dead_code))]
pub(crate) fn chime() -> Vec<u8> {
    const RATE: u32 = 44_100;
    let notes = [(880.0_f32, 0.0_f32), (1318.5, 0.11)];
    let length = (RATE as f32 * 0.45) as usize;
    let samples: Vec<i16> = (0..length)
        .map(|index| {
            let time = index as f32 / RATE as f32;
            let value: f32 = notes
                .iter()
                .filter(|(_, start)| time >= *start)
                .map(|(frequency, start)| {
                    let since = time - start;
                    // A soft attack, then a bell's fall.
                    let envelope = (since / 0.005).min(1.0) * (-since * 9.0).exp();
                    (std::f32::consts::TAU * frequency * since).sin() * envelope * 0.3
                })
                .sum();
            (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
        })
        .collect();
    let data = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1_u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_chime_is_a_short_wav() {
        let wav = chime();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(
            u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize,
            wav.len() - 8
        );
        assert!(wav.len() < MAX_SOUND_BYTES as usize);
        // Not silent.
        assert!(wav[44..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn a_notification_says_who_posted_it_when_clicked() {
        let identifier = identifier(7, "agents/list#main", Some("pane-3"));
        assert_eq!(
            parse_identifier(&identifier),
            Some(("agents/list#main".to_owned(), Some("pane-3".to_owned())))
        );
        assert_eq!(
            parse_identifier(&super::identifier(8, "agents", None)),
            Some(("agents".to_owned(), None))
        );
        assert_eq!(parse_identifier("someone else's"), None);
    }

    #[test]
    fn sounds_and_notifications_are_rate_limited() {
        let mut alerts = Alerts {
            last_sound: Some(Instant::now()),
            ..Alerts::default()
        };
        assert!(alerts.play(None).unwrap_err().contains("150 ms"));

        let mut alerts = Alerts::default();
        let notification = Notification {
            title: "Done".to_owned(),
            body: String::new(),
            id: None,
        };
        // Pretend the window already posted its burst.
        alerts.posted = std::iter::repeat_n(Instant::now(), NOTIFY_BURST).collect();
        assert!(
            alerts
                .notify("agents", &notification)
                .unwrap_err()
                .contains("more than 5")
        );
        alerts.posted = std::iter::repeat_n(
            Instant::now().checked_sub(NOTIFY_WINDOW).unwrap(),
            NOTIFY_BURST,
        )
        .collect();
        assert!(alerts.notify("agents", &notification).is_ok());
        assert_eq!(alerts.posted.len(), 1, "the old ones were forgotten");
    }
}
