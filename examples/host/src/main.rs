//! An app on the Moonframe library: its window is laid out by
//! `app/main.luau`, over Moonframe's components and providers, and Rust
//! adds two things Luau can't do:
//!
//! - `clock`, a native provider: the time, shared as `app.clock` and
//!   started by `app/providers/clock/provider.toml`;
//! - `counter`, a view the shell places with `ui.view`, which tells the
//!   shell each time it is clicked.
//!
//! `cargo run -p moonframe-host-example` opens it; `-- --check` loads it
//! headlessly; edits to `app/` show live.

use {
    moonframe::{
        Data, Handle, HostView, Messenger, Moonframe, Native, ViewInit,
        gpui_kit::{
            component::{ActiveTheme as _, button::Button},
            *,
        },
    },
    std::{
        process::ExitCode,
        time::{SystemTime, UNIX_EPOCH},
    },
};

fn main() -> ExitCode {
    Moonframe::new("host-example")
        .title("Host Example")
        .app_id("dev.moonframe.host-example")
        .version(env!("CARGO_PKG_VERSION"))
        .app_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/app"))
        .native("clock", Clock::default)
        .view("counter", Counter::new)
        .run()
}

/// Shares `{ time = "12:34:56", ticks = n }` each second; `{ reset = true }`
/// starts the ticks again.
#[derive(Default)]
struct Clock {
    ticks: i64,
}

impl Clock {
    fn publish(&self, host: &Handle) {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs() % 86_400);
        let time = format!(
            "{:02}:{:02}:{:02} UTC",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        );
        host.publish(Data::map([
            ("time", time.into()),
            ("ticks", self.ticks.into()),
        ]));
    }
}

impl Native for Clock {
    fn start(&mut self, host: &Handle) -> Result<(), String> {
        self.publish(host);
        Ok(())
    }

    fn tick(&mut self, host: &Handle) {
        self.ticks += 1;
        self.publish(host);
    }

    fn receive(&mut self, host: &Handle, _from: &str, message: Data) {
        if message.get("reset").and_then(Data::as_bool) == Some(true) {
            self.ticks = 0;
            self.publish(host);
        }
    }
}

/// A number and a button that adds `step` (a prop) to it, telling the
/// mod that drew it `{ counted = n }`.
struct Counter {
    count: i64,
    step: i64,
    label: SharedString,
    messenger: Messenger,
}

impl Counter {
    fn new(init: &ViewInit, _: &mut Window, _: &mut Context<Self>) -> Self {
        let mut counter = Self {
            count: 0,
            step: 1,
            label: SharedString::default(),
            messenger: init.messenger(),
        };
        counter.read(init.props());
        counter
    }

    fn read(&mut self, props: &Data) {
        self.step = props.get("step").and_then(Data::as_i64).unwrap_or(1);
        self.label = props
            .get("label")
            .and_then(Data::as_str)
            .unwrap_or("Counted in Rust")
            .to_owned()
            .into();
    }
}

impl HostView for Counter {
    fn set_props(&mut self, props: &Data, _: &mut Context<Self>) {
        self.read(props);
    }
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .size_full()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.label.clone()),
            )
            .child(div().text_3xl().child(self.count.to_string()))
            .child(
                Button::new("count")
                    .label(format!("+{}", self.step))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.count += this.step;
                        this.messenger
                            .send(Data::map([("counted", this.count.into())]));
                        cx.notify();
                    })),
            )
    }
}
