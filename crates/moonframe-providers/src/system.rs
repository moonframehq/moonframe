//! The `system` capability: this machine's load, measured between calls.

use {
    serde_json::json,
    std::time::Instant,
    sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System},
};

pub(crate) struct SystemLoad {
    system: System,
    networks: Networks,
    last: Instant,
}

impl SystemLoad {
    /// Fails where the system can't be measured at all.
    pub(crate) fn new() -> Result<Self, sysinfo::Error> {
        Ok(Self {
            system: System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing().with_usage())
                    .with_memory(MemoryRefreshKind::nothing().with_ram()),
            )?,
            networks: Networks::new_with_refreshed_list()?,
            last: Instant::now(),
        })
    }

    /// The load since the last call; CPU and network rates need two calls,
    /// so the first reads zero.
    pub(crate) fn sample(&mut self) -> serde_json::Value {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.networks.refresh(true);
        let elapsed = self.last.elapsed().as_secs_f64().max(0.001);
        self.last = Instant::now();
        let (used, total) = (self.system.used_memory(), self.system.total_memory());
        let rate = |bytes: u64| (bytes as f64 / elapsed).round() as u64;
        json!({
            "cpu": (f64::from(self.system.global_cpu_usage()) / 100.0).clamp(0.0, 1.0),
            "memory": used as f64 / total.max(1) as f64,
            "memory_used": used,
            "memory_total": total,
            "received": rate(self.networks.values().map(|data| data.received()).sum()),
            "transmitted": rate(self.networks.values().map(|data| data.transmitted()).sum()),
        })
    }
}
