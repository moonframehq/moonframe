//! Request logs, digested per route: a provider that shapes data, the work
//! wasm providers are for, written once for both builds.
//!
//! On `{ run = n }` it asks the host for `n` log lines through the
//! `bench.logs` capability (as a real provider would call an API), parses
//! each one, folds numeric path segments into `:id` so `/api/users/42` and
//! `/api/users/7` count together, and publishes per route: requests, error
//! rate, p50/p95/p99 latency and bytes, the busiest first.
//!
//! A line reads `2026-11-17T10:00:00Z GET /api/users/42 200 34ms 512B`.

use {
    moonframe_provider::{Provider, host_call},
    serde_json::{Value, json},
    std::collections::HashMap,
};

/// Routes published, the busiest first.
const TOP: usize = 50;

pub struct Digest {
    /// Cycles run, and lines digested in all.
    runs: u64,
    lines: u64,
}

#[derive(Default)]
struct Route {
    method: String,
    latencies: Vec<u32>,
    errors: u32,
    bytes: u64,
}

/// One line's fields, or none for a line that does not parse.
fn parse(line: &str) -> Option<(&str, &str, u16, u32, u64)> {
    let mut fields = line.split_ascii_whitespace();
    let _time = fields.next()?;
    let method = fields.next()?;
    let path = fields.next()?;
    let status = fields.next()?.parse().ok()?;
    let millis = fields.next()?.strip_suffix("ms")?.parse().ok()?;
    let bytes = fields.next()?.strip_suffix('B')?.parse().ok()?;
    Some((method, path, status, millis, bytes))
}

/// `/api/users/42/orders/7` as `/api/users/:id/orders/:id`.
fn route(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path);
    let mut route = String::with_capacity(path.len());
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        route.push('/');
        if segment.bytes().all(|byte| byte.is_ascii_digit()) {
            route.push_str(":id");
        } else {
            route.push_str(segment);
        }
    }
    route
}

/// The value at `fraction` of `sorted`.
fn percentile(sorted: &[u32], fraction: f64) -> u32 {
    let last = sorted.len().saturating_sub(1);
    sorted
        .get(((last as f64) * fraction).round() as usize)
        .copied()
        .unwrap_or(0)
}

impl Digest {
    fn digest(&mut self, lines: &[String]) -> Value {
        let mut routes: HashMap<String, Route> = HashMap::new();
        let mut unparsed = 0;
        for line in lines {
            let Some((method, path, status, millis, bytes)) = parse(line) else {
                unparsed += 1;
                continue;
            };
            let entry = routes.entry(route(path)).or_default();
            if entry.method.is_empty() {
                entry.method = method.to_owned();
            }
            entry.latencies.push(millis);
            entry.errors += u32::from(status >= 500);
            entry.bytes += bytes;
        }
        let mut busiest: Vec<(String, Route)> = routes.into_iter().collect();
        busiest.sort_by(|a, b| {
            b.1.latencies
                .len()
                .cmp(&a.1.latencies.len())
                .then(a.0.cmp(&b.0))
        });
        let routes_seen = busiest.len();
        let rows: Vec<Value> = busiest
            .into_iter()
            .take(TOP)
            .map(|(route, mut stats)| {
                stats.latencies.sort_unstable();
                let count = stats.latencies.len();
                json!({
                    "route": route,
                    "method": stats.method,
                    "requests": count,
                    "error_rate": f64::from(stats.errors) / count as f64,
                    "p50": percentile(&stats.latencies, 0.50),
                    "p95": percentile(&stats.latencies, 0.95),
                    "p99": percentile(&stats.latencies, 0.99),
                    "bytes": stats.bytes,
                })
            })
            .collect();
        self.runs += 1;
        self.lines += lines.len() as u64;
        json!({
            "run": self.runs,
            "lines": lines.len(),
            "total_lines": self.lines,
            "unparsed": unparsed,
            "routes_seen": routes_seen,
            "routes": rows,
        })
    }
}

impl Provider for Digest {
    fn start(_settings: Value) -> Result<Self, String> {
        Ok(Self { runs: 0, lines: 0 })
    }

    /// `{ run = n }`: fetch `n` lines, digest them, publish the result.
    /// `{ fetch = n }`: only fetch them, and publish how many came, to
    /// tell moving the data from digesting it.
    fn receive(&mut self, message: Value) -> Option<Value> {
        let (count, digest) = match (message.get("run"), message.get("fetch")) {
            (Some(count), _) => (count.as_u64()?, true),
            (None, Some(count)) => (count.as_u64()?, false),
            (None, None) => return None,
        };
        match host_call::<_, Vec<String>>("bench.logs", "batch", &json!({ "lines": count })) {
            Ok(lines) if digest => Some(self.digest(&lines)),
            Ok(lines) => Some(json!({ "fetched": lines.len() })),
            Err(problem) => Some(json!({ "problem": problem })),
        }
    }
}

moonframe_provider::export!(Digest);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_fold_ids_and_queries() {
        assert_eq!(
            route("/api/users/42/orders/7?x=1"),
            "/api/users/:id/orders/:id"
        );
        assert_eq!(route("/health"), "/health");
    }

    #[test]
    fn a_batch_is_digested_per_route() {
        let lines = [
            "t GET /api/users/1 200 10ms 100B",
            "t GET /api/users/2 503 30ms 50B",
            "t GET /api/users/3 200 20ms 10B",
            "t POST /login 200 5ms 1B",
            "not a line",
        ]
        .map(str::to_owned);
        let mut digest = Digest { runs: 0, lines: 0 };
        let out = digest.digest(&lines);
        assert_eq!(out["unparsed"], 1);
        assert_eq!(out["routes_seen"], 2);
        let users = &out["routes"][0];
        assert_eq!(users["route"], "/api/users/:id");
        assert_eq!(users["requests"], 3);
        assert_eq!(users["p50"], 20);
        assert_eq!(users["p99"], 30);
        assert_eq!(users["bytes"], 160);
        assert!((users["error_rate"].as_f64().unwrap_or(0.) - 1. / 3.).abs() < 1e-9);
    }
}
