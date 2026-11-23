//! The embedding host registers `example.echo`; see docs/wasm-providers.md.
//! The same crate runs as a wasm component or, linked into the host and
//! registered with `moonframe_providers::Portable`, as a native provider.
use {
    moonframe_provider::{Provider, host_call},
    serde_json::{Value, json},
};

pub struct Echo;

impl Provider for Echo {
    fn start(_: Value) -> Result<Self, String> {
        Ok(Self)
    }

    fn tick(&mut self) -> Option<Value> {
        host_call("example.echo", "echo", &json!({"hello": "miniapp"})).ok()
    }

    fn receive(&mut self, _: Value) -> Option<Value> {
        self.tick()
    }
}

moonframe_provider::export!(Echo);
