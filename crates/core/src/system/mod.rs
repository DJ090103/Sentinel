//! System information gathering: CPU, RAM, disk, OS, hostname.
//!
//! This backs the dashboard's system cards. Nothing here is persisted — see
//! `product-docs/ARCHITECTURE.md`, "Persistence": live system data is read fresh from
//! the OS on every refresh, because storing it would only produce a stale copy
//! of something the OS already tracks accurately.
//!
//! Everything that can be absent is modelled as `Option` rather than being
//! filled in with a placeholder. The dashboard is allowed to render "Unknown";
//! this module is not allowed to invent a value.

use std::path::Path;

use serde::Serialize;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

/// The shortest interval between two refreshes that still yields a meaningful
/// CPU reading.
///
/// Re-exported so callers scheduling a poll loop don't have to depend on
/// `sysinfo` directly just to learn the floor. Polling faster than this doesn't
/// produce fresher numbers — CPU usage is a delta between two samples, so there
/// has to be enough time between them to measure.
pub use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL as MINIMUM_REFRESH_INTERVAL;

/// Identity of the machine: what it's called and what it's running.
///
/// Every field is optional because each one genuinely can be unavailable —
/// containers frequently have no hostname, and OS version strings are missing
/// on some platforms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    /// Machine hostname, e.g. `workstation`.
    pub hostname: Option<String>,
    /// OS distribution name, e.g. `Ubuntu`.
    pub os_name: Option<String>,
    /// OS version, e.g. `24.04`.
    pub os_version: Option<String>,
    /// Kernel version, e.g. `6.8.0-40-generic`.
    pub kernel_version: Option<String>,
    /// Seconds since the machine booted, for the "a restart would help" hint.
    pub uptime_secs: u64,
}

/// Processor load at the moment of the snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    /// Average load across all cores, `0.0..=100.0`.
    ///
    /// This is a *delta* between the previous refresh and this one, which is
    /// why [`SystemMonitor`] is stateful. The very first snapshot taken from a
    /// freshly constructed monitor has no previous sample to compare against
    /// and will report `0.0` — see [`SystemMonitor::snapshot`].
    pub usage_percent: f32,
    /// Number of logical cores (including SMT/hyperthreads).
    pub core_count: usize,
}

/// Physical memory at the moment of the snapshot. All values in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    /// Total installed RAM.
    pub total_bytes: u64,
    /// RAM currently in use.
    pub used_bytes: u64,
    /// RAM available to new allocations.
    ///
    /// Deliberately "available" rather than "free": on Linux, reclaimable page
    /// cache counts as used but can be handed to a new allocation on demand, so
    /// "free" dramatically understates what a program can actually get.
    pub available_bytes: u64,
}

/// Space on the machine's primary filesystem. All values in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    /// Where the filesystem is mounted, e.g. `/` or `C:\`.
    pub mount_point: String,
    /// Capacity of the filesystem.
    pub total_bytes: u64,
    /// Space still available on it.
    pub available_bytes: u64,
}

/// A point-in-time reading of the host machine.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Machine identity and OS.
    pub host: HostInfo,
    /// Processor load.
    pub cpu: CpuInfo,
    /// Memory usage.
    pub memory: MemoryInfo,
    /// Primary filesystem, or `None` if no mounted filesystem was found.
    pub disk: Option<DiskInfo>,
}

/// Reads system metrics, keeping the state that CPU measurement requires.
///
/// This is stateful on purpose. CPU usage is not an instantaneous quantity: it
/// is the proportion of time the processor was busy *between two samples*. A
/// throwaway reader would have to either sleep internally (blocking its caller)
/// or report a meaningless figure. Holding the `sysinfo::System` between calls
/// means each snapshot is measured against the previous one, so a caller
/// polling every couple of seconds gets a real number without ever blocking.
///
/// Construct once, keep it alive, and call [`snapshot`](Self::snapshot)
/// repeatedly.
pub struct SystemMonitor {
    system: System,
    disks: Disks,
    refresh_kind: RefreshKind,
}

impl SystemMonitor {
    /// Creates a monitor and takes an initial baseline reading.
    ///
    /// Only CPU and memory are requested. The default `System::new_all()` also
    /// enumerates every process on the machine on every refresh, which is an
    /// enormous amount of work for the five numbers this module reports.
    pub fn new() -> Self {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
            .with_memory(MemoryRefreshKind::nothing().with_ram());

        Self {
            // Establishes the baseline that the first `snapshot()` measures
            // its CPU delta against.
            system: System::new_with_specifics(refresh_kind),
            disks: Disks::new_with_refreshed_list(),
            refresh_kind,
        }
    }

    /// Takes a fresh reading.
    ///
    /// CPU usage is measured against the previous call (or against
    /// construction, for the first call). If two calls are less than
    /// [`MINIMUM_REFRESH_INTERVAL`] apart the CPU figure will be `0.0` — there
    /// is no elapsed time to measure across. Callers polling on a timer of a
    /// second or more never encounter this; a caller that constructs a monitor
    /// and immediately snapshots will see `0.0` once.
    pub fn snapshot(&mut self) -> SystemInfo {
        self.system.refresh_specifics(self.refresh_kind);
        // `true` drops filesystems that are no longer mounted, rather than
        // continuing to report stale capacity for them.
        self.disks.refresh(true);

        SystemInfo {
            host: HostInfo {
                hostname: System::host_name(),
                os_name: System::name(),
                os_version: System::os_version(),
                kernel_version: System::kernel_version(),
                uptime_secs: System::uptime(),
            },
            cpu: CpuInfo {
                usage_percent: self.system.global_cpu_usage(),
                core_count: self.system.cpus().len(),
            },
            memory: MemoryInfo {
                total_bytes: self.system.total_memory(),
                used_bytes: self.system.used_memory(),
                available_bytes: self.system.available_memory(),
            },
            disk: primary_disk(&self.disks),
        }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Picks the one filesystem worth showing on a single "Disk" card.
///
/// Prefers the root filesystem (`/`), falling back to whichever mounted
/// filesystem is largest — which is what makes this work on Windows, where
/// there is no `/` and the system volume is normally the biggest.
///
/// Deliberately *not* a sum across all disks. On a typical Ubuntu machine every
/// installed snap appears as its own read-only `/dev/loop*` mount reporting a
/// full-size total and zero available space, so totalling them produces a disk
/// figure that is wildly wrong and gets worse the more software is installed.
fn primary_disk(disks: &Disks) -> Option<DiskInfo> {
    let list = disks.list();

    let disk = list
        .iter()
        .find(|disk| disk.mount_point() == Path::new("/"))
        .or_else(|| list.iter().max_by_key(|disk| disk.total_space()))?;

    Some(DiskInfo {
        mount_point: disk.mount_point().to_string_lossy().into_owned(),
        total_bytes: disk.total_space(),
        available_bytes: disk.available_space(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact values can't be asserted — they depend on the machine and change
    /// between calls. These check the invariants that would break if the
    /// wrong `sysinfo` accessor were wired to the wrong field, which is the
    /// realistic bug here.
    #[test]
    fn snapshot_reports_plausible_memory() {
        let mut monitor = SystemMonitor::new();
        let info = monitor.snapshot();

        assert!(info.memory.total_bytes > 0, "a machine has some RAM");
        assert!(
            info.memory.used_bytes <= info.memory.total_bytes,
            "used ({}) exceeded total ({})",
            info.memory.used_bytes,
            info.memory.total_bytes
        );
        assert!(
            info.memory.available_bytes <= info.memory.total_bytes,
            "available ({}) exceeded total ({})",
            info.memory.available_bytes,
            info.memory.total_bytes
        );
    }

    #[test]
    fn snapshot_reports_plausible_cpu() {
        let mut monitor = SystemMonitor::new();
        // Sleep past the floor so the second sample has elapsed time to
        // measure across; otherwise usage is definitionally 0.0.
        std::thread::sleep(MINIMUM_REFRESH_INTERVAL);
        let info = monitor.snapshot();

        assert!(info.cpu.core_count > 0, "a machine has at least one core");
        assert!(
            (0.0..=100.0).contains(&info.cpu.usage_percent),
            "cpu usage {} outside 0..=100",
            info.cpu.usage_percent
        );
    }

    #[test]
    fn disk_when_present_is_internally_consistent() {
        let mut monitor = SystemMonitor::new();
        let info = monitor.snapshot();

        // No assertion that a disk exists: a sufficiently locked-down
        // container legitimately reports none, and failing there would be a
        // false alarm rather than a real defect.
        if let Some(disk) = info.disk {
            assert!(
                disk.available_bytes <= disk.total_bytes,
                "available ({}) exceeded total ({}) on {}",
                disk.available_bytes,
                disk.total_bytes,
                disk.mount_point
            );
            assert!(!disk.mount_point.is_empty(), "mount point was empty");
        }
    }

    /// The whole reason `SystemMonitor` is stateful — it has to survive being
    /// polled over and over.
    #[test]
    fn monitor_is_reusable_across_snapshots() {
        let mut monitor = SystemMonitor::new();

        let first = monitor.snapshot();
        std::thread::sleep(MINIMUM_REFRESH_INTERVAL);
        let second = monitor.snapshot();

        assert_eq!(
            first.cpu.core_count, second.cpu.core_count,
            "core count changed between snapshots"
        );
        assert_eq!(
            first.memory.total_bytes, second.memory.total_bytes,
            "total RAM changed between snapshots"
        );
    }

    #[test]
    fn serializes_to_camel_case_for_the_frontend() {
        let info = SystemInfo {
            host: HostInfo {
                hostname: Some("workstation".to_owned()),
                os_name: None,
                os_version: None,
                kernel_version: None,
                uptime_secs: 3600,
            },
            cpu: CpuInfo {
                usage_percent: 12.5,
                core_count: 8,
            },
            memory: MemoryInfo {
                total_bytes: 100,
                used_bytes: 40,
                available_bytes: 60,
            },
            disk: None,
        };

        let json = serde_json::to_string(&info).expect("SystemInfo contains only plain data");

        assert!(json.contains("\"coreCount\":8"), "got {json}");
        assert!(json.contains("\"usagePercent\":12.5"), "got {json}");
        assert!(json.contains("\"totalBytes\":100"), "got {json}");
        // Absent values stay null rather than becoming a placeholder string.
        assert!(json.contains("\"osName\":null"), "got {json}");
        assert!(json.contains("\"disk\":null"), "got {json}");
    }
}
