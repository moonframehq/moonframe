//! The embedding host registers `example.echo`; see docs/wasm-providers.md.
use moonframe_provider::{bindings::Guest, host_call};

struct Echo;

impl Guest for Echo {
    fn start(_: String) -> Result<(), String> {
        Ok(())
    }

    fn tick() -> Option<String> {
        let answer: serde_json::Value = host_call(
            "example.echo",
            "echo",
            &serde_json::json!({"hello": "miniapp"}),
        )
        .ok()?;
        Some(answer.to_string())
    }

    fn receive(_: String) -> Option<String> {
        Self::tick()
    }
}

moonframe_provider::bindings::export!(Echo with_types_in moonframe_provider::bindings);
