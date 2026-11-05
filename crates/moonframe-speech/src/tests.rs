// The stub program and the login shell need a Unix shell.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use {
    super::{audio::*, *},
    moonframe_providers::Event,
    std::{
        io::Read,
        net::TcpListener,
        path::Path,
        sync::mpsc,
        time::{Duration, Instant},
    },
};

/// A microphone hearing a fixed take.
struct Fake {
    audio: Audio,
    refuse: Option<String>,
    problem: Shared<Option<String>>,
}

struct FakeTake {
    audio: Audio,
    problem: Shared<Option<String>>,
}

impl Microphone for Fake {
    fn open(&self, _: u32) -> Result<Box<dyn Take>, String> {
        match &self.refuse {
            Some(problem) => Err(problem.clone()),
            None => Ok(Box::new(FakeTake {
                audio: self.audio.clone(),
                problem: Arc::clone(&self.problem),
            })),
        }
    }
}

impl Take for FakeTake {
    fn level(&self) -> f32 {
        0.5
    }

    fn problem(&self) -> Option<String> {
        lock(&self.problem).clone()
    }

    fn finish(self: Box<Self>) -> Audio {
        self.audio
    }
}

fn speech_sound() -> Audio {
    Audio {
        samples: (0..16_000).map(|i| (i as f32 / 20.0).sin() * 0.3).collect(),
        rate: 16_000,
    }
}

struct Rig {
    speech: Speech,
    host: Handle,
    sent: Shared<Vec<(String, Value)>>,
    published: Shared<Vec<Value>>,
    problem: Shared<Option<String>>,
}

impl Rig {
    fn with(audio: Audio, refuse: Option<&str>, settings: Value) -> Self {
        let sent: Shared<Vec<(String, Value)>> = Arc::default();
        let published: Shared<Vec<Value>> = Arc::default();
        let (to_sent, to_published) = (Arc::clone(&sent), Arc::clone(&published));
        let host = Handle::new(
            "speech",
            Path::new("."),
            from_json(&settings).expect("settings"),
            Arc::new(move |event| match event {
                Event::Sent { to, msg, .. } => {
                    lock(&to_sent).push((to, to_json(&msg)));
                },
                Event::Published { data, .. } => lock(&to_published).push(to_json(&data)),
                _ => {},
            }),
        );
        let problem: Shared<Option<String>> = Arc::default();
        let mut speech = Speech::new(Box::new(Fake {
            audio,
            refuse: refuse.map(str::to_owned),
            problem: Arc::clone(&problem),
        }));
        speech.start(&host).expect("started");
        let mut rig = Self {
            speech,
            host,
            sent,
            published,
            problem,
        };
        rig.until(|rig| lock(&rig.speech.statuses).is_some());
        // The check ended after that tick's look at it: publish what it found.
        rig.speech.tick(&rig.host);
        rig
    }

    fn new(settings: Value) -> Self {
        Self::with(speech_sound(), None, settings)
    }

    fn tell(&mut self, from: &str, message: Value) {
        let data = from_json(&message).expect("message");
        self.speech.receive(&self.host, from, data);
    }

    fn state(&self) -> Value {
        lock(&self.published).last().cloned().unwrap_or_default()
    }

    fn until(&mut self, done: impl Fn(&Self) -> bool) {
        let started = Instant::now();
        while !done(self) {
            assert!(started.elapsed() < Duration::from_secs(10), "timed out");
            self.speech.tick(&self.host);
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// The answer of `action` sent to `mod_name`, waiting for it.
    fn answer(&mut self, mod_name: &str, action: &str) -> Value {
        let found = |rig: &Self| {
            lock(&rig.sent)
                .iter()
                .find(|(to, value)| to == mod_name && value["action"] == action)
                .map(|(_, value)| value.clone())
        };
        self.until(|rig| found(rig).is_some());
        found(self).expect("answered")
    }
}

#[cfg(unix)]
fn stub(body: &str) -> (tempfile::TempDir, Value) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("folder");
    let path = dir.path().join("stt");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let settings = json!({
        "recorders": ["editor"],
        "prefer": ["local"],
        "engines": { "local": {
            "kind": "command",
            "command": [path.to_string_lossy(), "{file}"],
        } },
    });
    (dir, settings)
}

#[test]
fn a_rate_change_keeps_the_length_and_the_level() {
    let samples = vec![0.5; 48_000];
    let down = resample(&samples, 48_000, RATE);
    assert_eq!(down.len(), 16_000);
    assert!(down.iter().all(|sample| (sample - 0.5).abs() < 1e-6));
    let up = resample(&[0.0, 1.0], 8_000, 16_000);
    assert_eq!(up, [0.0, 0.5, 1.0, 1.0]);
    assert!(resample(&[], 48_000, RATE).is_empty());
}

#[test]
fn a_wav_says_what_it_holds() {
    let bytes = wav(&[1.0, -1.0, 0.0, 2.0], RATE);
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(bytes.len(), 44 + 8);
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(
        u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
        RATE
    );
    assert_eq!(
        u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
        8
    );
    assert_eq!(i16::from_le_bytes([bytes[44], bytes[45]]), i16::MAX);
    // Past full scale is clipped, never wrapped round.
    assert_eq!(i16::from_le_bytes([bytes[50], bytes[51]]), i16::MAX);
}

#[test]
fn the_level_follows_decibels() {
    assert_eq!(level(&[]), 0.0);
    assert_eq!(level(&[0.0; 8]), 0.0);
    assert!((level(&[1.0; 8]) - 1.0).abs() < 1e-6);
    let quiet = level(&[0.03; 8]);
    assert!(quiet > 0.4 && quiet < 0.6, "{quiet}");
}

#[test]
fn only_a_listed_mod_may_listen() {
    let mut rig = Rig::new(json!({ "recorders": ["editor"] }));
    rig.tell("status", json!({ "start": true, "id": "mine" }));
    let answer = rig.answer("status", "start");
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["reply"], "mine");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("recorders"))
    );
    assert_eq!(rig.state()["state"], "idle");
}

#[test]
fn with_nothing_ready_it_says_so() {
    let mut rig = Rig::new(json!({ "recorders": ["editor"] }));
    assert_eq!(rig.state()["available"], false);
    rig.tell("editor", json!({ "start": true }));
    let answer = rig.answer("editor", "start");
    assert_eq!(answer["ok"], false);
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("No speech engine"))
    );
}

#[test]
fn a_remote_engine_is_never_chosen_unless_preferred() {
    let settings = json!({
        "recorders": ["editor"],
        "engines": { "cloud": { "kind": "remote", "url": "https://example.com/v1", "model": "m" } },
    });
    let mut rig = Rig::new(settings);
    let engines = rig.state()["engines"].clone();
    assert_eq!(engines[0]["ready"], true);
    assert_eq!(engines[0]["remote"], true);
    assert_eq!(rig.state()["available"], false);
    rig.tell("editor", json!({ "start": true }));
    assert_eq!(rig.answer("editor", "start")["ok"], false);
}

#[test]
fn engines_that_make_no_sense_are_left_out() {
    let settings = from_json(&json!({ "engines": {
        "plain": { "kind": "remote", "url": "http://example.com/v1", "model": "m" },
        "lan": { "kind": "remote", "url": "http://localhost.example.com:80", "model": "m" },
        "no-file": { "kind": "command", "command": ["stt", "take.wav"] },
        "odd": { "kind": "psychic" },
        "shell-key": { "kind": "remote", "url": "https://a.b", "model": "m", "key_env": "A;rm" },
        "local": { "kind": "remote", "url": "http://127.0.0.1:8000/v1/", "model": "m" },
    } }))
    .expect("settings");
    let engines = Engines::read(&settings);
    let names: Vec<&str> = engines
        .all
        .iter()
        .map(|engine| engine.name.as_str())
        .collect();
    assert_eq!(names, ["local"]);
    assert_eq!(engines.problems.len(), 5);
}

#[cfg(unix)]
#[test]
fn a_take_goes_through_a_local_program_to_the_mod_that_asked() {
    // The program proves it got a real WAV by reading its header.
    let (_dir, settings) = stub("head -c 4 \"$1\"; echo ' hello there '");
    let mut rig = Rig::new(settings);
    assert_eq!(rig.state()["available"], true);
    assert_eq!(rig.state()["engine"], "local");
    rig.tell("editor/dictation#a", json!({ "start": true, "id": "mine" }));
    assert_eq!(rig.answer("editor/dictation#a", "start")["ok"], true);
    assert_eq!(rig.state()["state"], "recording");
    assert_eq!(rig.state()["owner"], "editor");
    assert_eq!(rig.state()["level"], 0.5);
    rig.tell("editor/dictation#a", json!({ "stop": true }));
    let answer = rig.answer("editor/dictation#a", "transcript");
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["reply"], "mine");
    assert_eq!(answer["text"], "RIFF hello there");
    assert_eq!(answer["engine"], "local");
    assert_eq!(rig.state()["state"], "idle");
    // What was said stays between the provider and that mod.
    let shared = lock(&rig.published)
        .iter()
        .map(Value::to_string)
        .collect::<String>();
    assert!(!shared.contains("hello there"));
}

#[cfg(unix)]
#[test]
fn another_mod_cannot_stop_a_take_or_start_a_second() {
    let (_dir, mut settings) = stub("echo hi");
    settings["recorders"] = json!(["editor", "notes"]);
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("notes", json!({ "stop": true }));
    rig.tell("notes", json!({ "cancel": true }));
    assert_eq!(rig.state()["state"], "recording");
    rig.tell("notes", json!({ "start": true }));
    assert!(
        rig.answer("notes", "start")["error"]
            .as_str()
            .is_some_and(|e| e.contains("Already"))
    );
}

#[cfg(unix)]
#[test]
fn a_cancelled_take_answers_once_and_stops_the_program() {
    let (_dir, settings) = stub("sleep 30");
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    assert_eq!(rig.state()["state"], "transcribing");
    rig.tell("editor", json!({ "cancel": true }));
    let answer = rig.answer("editor", "transcript");
    assert_eq!(answer["cancelled"], true);
    assert_eq!(rig.state()["state"], "idle");
    thread::sleep(Duration::from_millis(150));
    rig.speech.tick(&rig.host);
    let transcripts = lock(&rig.sent)
        .iter()
        .filter(|(_, value)| value["action"] == "transcript")
        .count();
    assert_eq!(transcripts, 1);
}

#[cfg(unix)]
#[test]
fn a_failing_program_tells_why() {
    let (_dir, settings) = stub("echo 'no model loaded' >&2; exit 3");
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    let answer = rig.answer("editor", "transcript");
    assert_eq!(answer["ok"], false);
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("no model loaded"))
    );
}

#[cfg(unix)]
#[test]
fn silence_and_slips_never_reach_an_engine() {
    let (dir, settings) = stub("touch \"$(dirname \"$0\")/ran\"; echo words");
    let silent = Audio {
        samples: vec![0.0; 32_000],
        rate: 16_000,
    };
    let mut rig = Rig::with(silent, None, settings.clone());
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    let answer = rig.answer("editor", "transcript");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("privacy"))
    );

    let slip = Audio {
        samples: vec![0.5; 800],
        rate: 16_000,
    };
    let mut rig = Rig::with(slip, None, settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    let answer = rig.answer("editor", "transcript");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.contains("too short"))
    );
    assert!(!dir.path().join("ran").exists());
}

#[cfg(unix)]
#[test]
fn a_microphone_that_refuses_or_fails_is_answered() {
    let (_dir, settings) = stub("echo hi");
    let mut rig = Rig::with(
        speech_sound(),
        Some("No microphone found"),
        settings.clone(),
    );
    rig.tell("editor", json!({ "start": true }));
    assert_eq!(
        rig.answer("editor", "start")["error"],
        "No microphone found"
    );
    assert_eq!(rig.state()["state"], "idle");

    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    *lock(&rig.problem) = Some("unplugged".into());
    assert_eq!(rig.answer("editor", "transcript")["error"], "unplugged");
    assert_eq!(rig.state()["state"], "idle");
}

#[cfg(unix)]
#[test]
fn a_take_past_the_limit_transcribes_by_itself() {
    let (_dir, mut settings) = stub("echo done");
    settings["max_seconds"] = json!(1);
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    assert_eq!(rig.answer("editor", "transcript")["text"], "done");
}

#[cfg(unix)]
#[test]
fn letting_go_ends_a_held_take_and_leaves_a_tapped_one_listening() {
    let (_dir, settings) = stub("echo held");
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "release": true }));
    assert_eq!(rig.state()["state"], "recording");
    // Another mod's button coming up ends nothing.
    thread::sleep(HOLD);
    rig.tell("notes", json!({ "release": true }));
    assert_eq!(rig.state()["state"], "recording");
    rig.tell("editor", json!({ "release": true }));
    assert_eq!(rig.answer("editor", "transcript")["text"], "held");
}

#[cfg(unix)]
#[test]
fn the_release_of_the_tap_that_stopped_a_take_does_nothing() {
    let (_dir, settings) = stub("sleep 0.2; echo tapped");
    let mut rig = Rig::new(settings);
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    rig.tell("editor", json!({ "release": true }));
    assert_eq!(rig.state()["state"], "transcribing");
    assert_eq!(rig.answer("editor", "transcript")["text"], "tapped");
}

/// One request answered with `status` and `body`; hands back what it read.
fn server(status: &str, body: &str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
    let url = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().expect("port").port()
    );
    let (got, request) = mpsc::channel();
    let answer = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    thread::spawn(move || {
        use std::io::Write as _;
        let (mut stream, _) = listener.accept().expect("connected");
        let mut bytes = Vec::new();
        let mut chunk = [0; 8192];
        // The take's closing boundary ends what the client sends.
        while !bytes.windows(4).any(|end| end == b"--\r\n") {
            let read = stream.read(&mut chunk).expect("read");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        stream.write_all(answer.as_bytes()).expect("answered");
        let _ = got.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    (url, request)
}

fn remote_settings(url: &str) -> Value {
    json!({
        "recorders": ["editor"],
        "prefer": ["cloud"],
        "engines": { "cloud": { "kind": "remote", "url": url, "model": "tiny", "language": "fr" } },
    })
}

#[test]
fn a_remote_service_gets_the_take_and_its_text_comes_back() {
    let (url, request) = server("200 OK", r#"{"text":" bonjour "}"#);
    let mut rig = Rig::new(remote_settings(&url));
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    assert_eq!(rig.answer("editor", "transcript")["text"], "bonjour");
    let request = request.recv().expect("request");
    assert!(request.starts_with("POST /v1/audio/transcriptions "));
    assert!(request.contains("name=\"model\"\r\n\r\ntiny\r\n"));
    assert!(request.contains("name=\"language\"\r\n\r\nfr\r\n"));
    assert!(request.contains("filename=\"speech.wav\""));
    assert!(request.contains("RIFF"));
}

#[test]
fn a_remote_refusal_shows_the_services_words() {
    let (url, _request) = server("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#);
    let mut rig = Rig::new(remote_settings(&url));
    rig.tell("editor", json!({ "start": true }));
    rig.tell("editor", json!({ "stop": true }));
    let answer = rig.answer("editor", "transcript");
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["error"], "The speech service refused (401): bad key");
}

#[test]
fn the_shipped_manifest_reads_and_keeps_remote_engines_out_of_prefer() {
    let text = include_str!("../../../app/providers/speech/provider.toml");
    let manifest: toml::Value = toml::from_str(text).expect("manifest");
    let settings: Value = serde_json::to_value(&manifest["settings"]).expect("json");
    let engines = Engines::read(&from_json(&settings).expect("settings"));
    assert!(engines.problems.is_empty(), "{:?}", engines.problems);
    assert!(engines.named("apple").is_some());
    for name in settings["prefer"].as_array().expect("prefer") {
        let engine = engines.named(name.as_str().expect("name")).expect("listed");
        assert!(!engine.publish(None)["remote"].as_bool().expect("flag"));
    }
}

/// Touching the recognizer's authorization aborts a process with no usage
/// string, which a test binary and `just run` both are.
#[cfg(target_os = "macos")]
#[test]
fn apple_outside_a_bundle_says_why_instead_of_aborting() {
    let rig = Rig::new(json!({
        "recorders": ["editor"],
        "prefer": ["apple"],
        "engines": { "apple": { "kind": "apple" } },
    }));
    let engine = rig.state()["engines"][0].clone();
    assert_eq!(engine["ready"], false);
    assert!(
        engine["reason"]
            .as_str()
            .is_some_and(|why| why.contains("installed app"))
    );
    assert_eq!(rig.state()["available"], false);
}
