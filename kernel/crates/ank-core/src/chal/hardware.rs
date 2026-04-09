use std::time::{Duration, Instant};
use sysinfo::System;

/// Aegis Hardware Monitor: Collects real-time telemetry from the host OS.
/// Follows SRE principles for non-blocking collection.
pub struct HardwareMonitor {
    sys: System,
    last_refresh: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct HardwareStatus {
    pub cpu_usage: f32,
    pub total_mem_mb: u64,
    pub used_mem_mb: u64,
    pub uptime_secs: u64,
}

impl HardwareMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            sys,
            last_refresh: Instant::now(),
        }
    }
}

impl Default for HardwareMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareMonitor {
    /// Pulls fresh metrics if the cache is older than 2 seconds.
    pub fn get_status(&mut self) -> HardwareStatus {
        if self.last_refresh.elapsed() > Duration::from_secs(2) {
            self.sys.refresh_cpu_usage();
            self.sys.refresh_memory();
            self.last_refresh = Instant::now();
        }

        HardwareStatus {
            cpu_usage: self.sys.global_cpu_usage(),
            total_mem_mb: self.sys.total_memory() / 1024 / 1024,
            used_mem_mb: self.sys.used_memory() / 1024 / 1024,
            uptime_secs: System::uptime(),
        }
    }
}
