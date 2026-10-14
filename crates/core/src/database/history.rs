//! Reading and writing the history tables that the Insights page charts.
//!
//! Plain functions over a `&Connection` so callers decide how the connection is
//! shared. Sizes are stored as SQLite integers (i64); a disk would need to
//! exceed 8 EiB to overflow, and conversions saturate rather than wrap.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use sentinel_plugin_api::CleanOutcome;
use serde::Serialize;

use crate::system::SystemInfo;

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// Stores one reading of the machine.
///
/// `cpu_percent` is `None` when the reading has no earlier sample to measure
/// against — a CPU figure is a delta, and a first sample would record a false 0.
pub fn record_snapshot(
    conn: &Connection,
    info: &SystemInfo,
    cpu_percent: Option<f32>,
    at: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let disk = info.disk.as_ref();
    conn.execute(
        "INSERT INTO system_snapshots
         (taken_at, cpu_percent, ram_used_bytes, ram_total_bytes, disk_used_bytes, disk_total_bytes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            at.to_rfc3339(),
            cpu_percent,
            to_i64(info.memory.used_bytes),
            to_i64(info.memory.total_bytes),
            disk.map(|d| to_i64(d.total_bytes.saturating_sub(d.available_bytes))),
            disk.map(|d| to_i64(d.total_bytes)),
        ],
    )?;
    Ok(())
}

/// Stores the result of one plugin's clean, so the history is Sentinel's own
/// record of what it actually removed (measured bytes, not the estimate).
pub fn record_cleanup(
    conn: &Connection,
    plugin_id: &str,
    outcome: &CleanOutcome,
    at: DateTime<Utc>,
) -> rusqlite::Result<()> {
    // Serializing a `Vec<String>` cannot fail; the fallback keeps the row.
    let removed = serde_json::to_string(&outcome.removed).unwrap_or_else(|_| "[]".to_owned());
    conn.execute(
        "INSERT INTO cleanup_history
         (cleaned_at, plugin_id, removed_json, failure_count, reclaimed_bytes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            at.to_rfc3339(),
            plugin_id,
            removed,
            to_i64(outcome.failures.len() as u64),
            to_i64(outcome.reclaimed_bytes),
        ],
    )?;
    Ok(())
}

/// Adds `seconds` of active time to `day` (local `YYYY-MM-DD`) for an activity
/// and app. `activity` is `"other"` when the foreground app was unclassified;
/// `app` is empty when unknown. Days older than 30 are pruned.
pub fn record_usage(
    conn: &Connection,
    day: &str,
    activity: Option<&str>,
    app: Option<&str>,
    seconds: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_usage (day, activity, app, seconds) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(day, activity, app) DO UPDATE SET seconds = seconds + excluded.seconds",
        params![
            day,
            activity.unwrap_or("other"),
            app.unwrap_or(""),
            to_i64(seconds)
        ],
    )?;
    conn.execute(
        "DELETE FROM app_usage WHERE day < (SELECT date(?1, '-30 days'))",
        params![day],
    )?;
    Ok(())
}

/// One day's active seconds for one activity and app.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    /// Local date, `YYYY-MM-DD`.
    pub day: String,
    /// Activity name, or `other`.
    pub activity: String,
    /// Process name, or empty.
    pub app: String,
    /// Active seconds.
    pub seconds: u64,
}

/// All usage rows on or after `since_day`.
pub fn usage_since(conn: &Connection, since_day: &str) -> rusqlite::Result<Vec<UsageRow>> {
    let mut stmt = conn.prepare(
        "SELECT day, activity, app, seconds FROM app_usage WHERE day >= ?1 ORDER BY day",
    )?;
    let rows = stmt
        .query_map(params![since_day], |row| {
            Ok(UsageRow {
                day: row.get(0)?,
                activity: row.get(1)?,
                app: row.get(2)?,
                seconds: to_u64(row.get(3)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// One stored machine reading.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPoint {
    /// RFC 3339.
    pub taken_at: String,
    /// `None` when no earlier sample existed.
    pub cpu_percent: Option<f64>,
    /// RAM in use.
    pub ram_used_bytes: u64,
    /// Total RAM.
    pub ram_total_bytes: u64,
    /// Primary disk in use, if one was found.
    pub disk_used_bytes: Option<u64>,
    /// Primary disk capacity, if one was found.
    pub disk_total_bytes: Option<u64>,
}

/// One recorded clean.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRecord {
    /// RFC 3339.
    pub cleaned_at: String,
    /// Which plugin cleaned.
    pub plugin_id: String,
    /// Labels of what was removed.
    pub removed: Vec<String>,
    /// How many targets failed.
    pub failure_count: u64,
    /// Bytes actually freed.
    pub reclaimed_bytes: u64,
}

/// Everything the Insights page shows.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    /// Readings in the window, oldest first.
    pub snapshots: Vec<SnapshotPoint>,
    /// Most recent cleans, newest first (capped).
    pub cleanups: Vec<CleanupRecord>,
    /// All-time bytes freed.
    pub total_reclaimed_bytes: u64,
    /// All-time number of cleans.
    pub clean_count: u64,
}

/// [`record_cleanup`] stamped with the current time.
pub fn record_cleanup_now(
    conn: &Connection,
    plugin_id: &str,
    outcome: &CleanOutcome,
) -> rusqlite::Result<()> {
    record_cleanup(conn, plugin_id, outcome, Utc::now())
}

/// The last 30 days of readings plus the cleanup history, as of now.
pub fn insights_now(conn: &Connection) -> rusqlite::Result<Insights> {
    insights(conn, 30, Utc::now())
}

const MAX_CLEANUPS: i64 = 365;
const MAX_SNAPSHOTS: i64 = 5000;

/// Loads the Insights data for the last `days` days of readings.
pub fn insights(conn: &Connection, days: i64, now: DateTime<Utc>) -> rusqlite::Result<Insights> {
    let since = (now - Duration::days(days)).to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT taken_at, cpu_percent, ram_used_bytes, ram_total_bytes, disk_used_bytes, disk_total_bytes
         FROM (SELECT * FROM system_snapshots WHERE taken_at >= ?1 ORDER BY taken_at DESC LIMIT ?2)
         ORDER BY taken_at ASC",
    )?;
    let snapshots = stmt
        .query_map(params![since, MAX_SNAPSHOTS], |row| {
            Ok(SnapshotPoint {
                taken_at: row.get(0)?,
                cpu_percent: row.get(1)?,
                ram_used_bytes: to_u64(row.get(2)?),
                ram_total_bytes: to_u64(row.get(3)?),
                disk_used_bytes: row.get::<_, Option<i64>>(4)?.map(to_u64),
                disk_total_bytes: row.get::<_, Option<i64>>(5)?.map(to_u64),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut stmt = conn.prepare(
        "SELECT cleaned_at, plugin_id, removed_json, failure_count, reclaimed_bytes
         FROM cleanup_history ORDER BY cleaned_at DESC LIMIT ?1",
    )?;
    let cleanups = stmt
        .query_map(params![MAX_CLEANUPS], |row| {
            let removed_json: String = row.get(2)?;
            Ok(CleanupRecord {
                cleaned_at: row.get(0)?,
                plugin_id: row.get(1)?,
                removed: serde_json::from_str(&removed_json).unwrap_or_default(),
                failure_count: to_u64(row.get(3)?),
                reclaimed_bytes: to_u64(row.get(4)?),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let (total, count): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(reclaimed_bytes), 0), COUNT(*) FROM cleanup_history",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    Ok(Insights {
        snapshots,
        cleanups,
        total_reclaimed_bytes: to_u64(total),
        clean_count: to_u64(count),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::open_in_memory;
    use crate::system::SystemMonitor;

    #[test]
    fn usage_accumulates_per_day_activity_and_app() {
        let conn = open_in_memory().expect("open");
        record_usage(&conn, "2026-10-14", Some("dev"), Some("code"), 3).expect("first");
        record_usage(&conn, "2026-10-14", Some("dev"), Some("code"), 4).expect("second");
        record_usage(&conn, "2026-10-14", None, None, 1).expect("unclassified");

        let rows = usage_since(&conn, "2026-10-01").expect("rows");

        assert_eq!(rows.len(), 2);
        let dev = rows.iter().find(|r| r.activity == "dev").expect("dev row");
        assert_eq!(dev.seconds, 7);
        assert_eq!(
            rows.iter()
                .find(|r| r.activity == "other")
                .expect("other")
                .app,
            ""
        );
    }

    #[test]
    fn records_round_trip_and_totals_add_up() {
        let conn = open_in_memory().expect("open");
        let now = Utc::now();
        let info = SystemMonitor::new().snapshot();

        record_snapshot(&conn, &info, None, now - Duration::days(40)).expect("old snapshot");
        record_snapshot(&conn, &info, Some(12.5), now).expect("snapshot");
        for bytes in [100, 250] {
            let outcome = CleanOutcome {
                removed: vec!["npm cache".to_owned()],
                reclaimed_bytes: bytes,
                failures: vec![],
            };
            record_cleanup(&conn, "npm", &outcome, now).expect("cleanup");
        }

        let result = insights(&conn, 30, now).expect("insights");

        assert_eq!(
            result.snapshots.len(),
            1,
            "the 40-day-old reading is outside the window"
        );
        assert_eq!(result.snapshots[0].cpu_percent, Some(12.5));
        assert_eq!(result.total_reclaimed_bytes, 350);
        assert_eq!(result.clean_count, 2);
        assert_eq!(result.cleanups[0].removed, ["npm cache"]);
    }
}
