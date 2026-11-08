//! A stand-in for a model: an OpenAI-compatible chat completions server that
//! streams scripted answers, so the real pi runs end to end with no account.
//! What it answers depends on the words in the last prompt:
//!
//! - "fix": reads price.js, edits it, runs a command, then sums up
//! - "slow": streams slowly, for steering and stopping a run
//! - "title": a thread's title (what the app asks for after a first run)
//! - else: a short markdown answer

use {
    serde_json::{Value, json},
    std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    },
};

/// One scripted answer.
#[derive(Default)]
struct Step {
    think: String,
    text: String,
    tool: Option<(&'static str, Value)>,
    delay: Option<Duration>,
}

fn text_of(message: &Value) -> String {
    match &message["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|part| part["text"].as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn last_prompt(messages: &[Value]) -> String {
    let user = messages
        .iter()
        .rev()
        .find(|message| message["role"] == "user");
    user.map(text_of).unwrap_or_default()
}

/// The tool calls already answered since the last prompt.
fn tools_since_prompt(messages: &[Value]) -> usize {
    let recent = messages
        .iter()
        .rev()
        .take_while(|message| message["role"] != "user");
    recent.filter(|message| message["role"] == "tool").count()
}

fn capitalised(word: &str) -> String {
    let mut letters = word.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(letters).collect()
    })
}

fn script(messages: &[Value]) -> Step {
    let said = last_prompt(messages);
    let prompt = said.to_lowercase();
    if prompt.contains("thread title") || prompt.contains("short title") {
        let asked = said.split("\"text\":\"").nth(1).unwrap_or("New thread");
        let asked = asked.split('"').next().unwrap_or_default();
        let words: Vec<String> = asked.split_whitespace().take(4).map(capitalised).collect();
        return Step {
            text: words.join(" "),
            ..Step::default()
        };
    }
    if prompt.contains("fix") {
        return match tools_since_prompt(messages) {
            0 => Step {
                think: "The cents lose their leading zero.".to_owned(),
                tool: Some(("read", json!({ "path": "src/price.js" }))),
                ..Step::default()
            },
            1 => Step {
                tool: Some((
                    "edit",
                    json!({
                        "path": "src/price.js",
                        "edits": [{
                            "oldText": "${rest}",
                            "newText": "${String(rest).padStart(2, \"0\")}",
                        }],
                    }),
                )),
                ..Step::default()
            },
            2 => Step {
                tool: Some(("bash", json!({ "command": "echo all tests pass" }))),
                ..Step::default()
            },
            _ => Step {
                text: "Fixed `formatPrice` to pad cents to two digits.\n\nThe tests pass."
                    .to_owned(),
                ..Step::default()
            },
        };
    }
    if prompt.contains("slow") {
        return Step {
            text: "Thinking this through one word at a time. ".repeat(40),
            delay: Some(Duration::from_millis(60)),
            ..Step::default()
        };
    }
    Step {
        text: "Here is a **short** answer with `code`.\n\n- one\n- two\n\n```js\nconsole.log('hi');\n```\n"
            .to_owned(),
        ..Step::default()
    }
}

fn chunk(model: &str, delta: &Value, finish: Option<&str>) -> String {
    let record = json!({
        "id": "mock",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": model,
        "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }],
    });
    format!("data: {record}\n\n")
}

/// `text`'s words, each with the spaces after it.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive(' ')
}

fn answer(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    let request: Value = serde_json::from_str(body).unwrap_or_default();
    let messages = request["messages"].as_array().cloned().unwrap_or_default();
    let model = request["model"].as_str().unwrap_or("mock").to_owned();
    let step = script(&messages);
    let delay = step.delay.unwrap_or(Duration::from_millis(15));
    let head = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
    stream.write_all(head.as_bytes())?;
    let mut send = |text: String| {
        stream.write_all(text.as_bytes())?;
        stream.flush()
    };
    send(chunk(
        &model,
        &json!({ "role": "assistant", "content": "" }),
        None,
    ))?;
    for word in words(&step.think) {
        send(chunk(&model, &json!({ "reasoning_content": word }), None))?;
        thread::sleep(delay);
    }
    for word in words(&step.text) {
        send(chunk(&model, &json!({ "content": word }), None))?;
        thread::sleep(delay);
    }
    if let Some((name, args)) = &step.tool {
        let call = json!({ "tool_calls": [{
            "index": 0,
            "id": format!("call_{name}_{}", messages.len()),
            "type": "function",
            "function": { "name": name, "arguments": args.to_string() },
        }]});
        send(chunk(&model, &call, None))?;
    }
    let finish = if step.tool.is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    send(chunk(&model, &json!({}), Some(finish)))?;
    let usage = json!({
        "id": "mock",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": model,
        "choices": [],
        "usage": { "prompt_tokens": 1200, "completion_tokens": 80, "total_tokens": 1280 },
    });
    send(format!("data: {usage}\n\n"))?;
    send("data: [DONE]\n\n".to_owned())
}

fn serve_one(mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    answer(&mut stream, &String::from_utf8_lossy(&body))
}

/// Serves on `port` (0 for any), for as long as the process lives; gives
/// the port.
pub fn serve(port: u16) -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                // A client that goes mid-answer is how a run stops.
                let _ = serve_one(stream);
            });
        }
    });
    Ok(port)
}
