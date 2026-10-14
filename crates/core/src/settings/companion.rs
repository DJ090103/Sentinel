//! Settings for the desktop companion: the floating pixel monitor.
//!
//! Every value arrives from the webview or a hand-edited JSON file, so nothing
//! here is trusted: [`CompanionSettings::sanitized`] clamps numbers, validates
//! colours and bounds text, and is applied to anything that enters the store.

use serde::{Deserialize, Serialize};

/// The monitor's body shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// The default boxy-with-curves monitor.
    #[default]
    Classic,
    /// Softer corners.
    Rounded,
    /// Square corners.
    Boxy,
    /// Retro TV with antennae.
    Tv,
    /// A monitor with cat ears.
    Cat,
}

/// Colours and silhouette, all as `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Avatar {
    /// Casing colour.
    pub body: String,
    /// Eye and mouth glow colour.
    pub eyes: String,
    /// Screen background colour.
    pub screen: String,
    /// Body shape.
    pub shape: Shape,
}

impl Default for Avatar {
    fn default() -> Self {
        Self {
            body: "#e6dfcc".to_owned(),
            eyes: "#7dffd0".to_owned(),
            screen: "#0c2426".to_owned(),
            shape: Shape::Classic,
        }
    }
}

/// When the companion starts to look stressed. Percentages except `apps_*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Thresholds {
    /// CPU % that reads as "working".
    pub cpu_medium: u32,
    /// CPU % that reads as "busy".
    pub cpu_high: u32,
    /// CPU % that reads as "overloaded".
    pub cpu_very_high: u32,
    /// Memory % that reads as "busy".
    pub mem_high: u32,
    /// Memory % that reads as "overloaded".
    pub mem_very_high: u32,
    /// Open apps that read as "busy".
    pub apps_high: u32,
    /// Open apps that read as "overloaded".
    pub apps_very_high: u32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            cpu_medium: 35,
            cpu_high: 65,
            cpu_very_high: 85,
            mem_high: 90,
            mem_very_high: 97,
            apps_high: 9,
            apps_very_high: 14,
        }
    }
}

/// Everything configurable about the companion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompanionSettings {
    /// Whether the companion window is shown.
    pub enabled: bool,
    /// Name shown above its head (1 to 24 characters).
    pub name: String,
    /// Screen pixels per art pixel, 2 to 12.
    pub scale: u32,
    /// Keep it above other windows.
    pub always_on_top: bool,
    /// Start Sentinel when the user logs in.
    pub launch_at_login: bool,
    /// React to the foreground app (coding, gaming, watching...).
    pub work_mode: bool,
    /// Classify browser tabs by window title. Titles stay in memory.
    pub read_titles: bool,
    /// Seconds without input before it falls asleep, 10 to 86400.
    pub sleep_after_secs: u32,
    /// Milliseconds between sensor readings, 1000 to 15000.
    pub poll_ms: u32,
    /// Load levels.
    pub thresholds: Thresholds,
    /// Look.
    pub avatar: Avatar,
    /// Last window position, if it has been dragged.
    pub x: Option<i32>,
    /// Last window position, if it has been dragged.
    pub y: Option<i32>,
}

impl Default for CompanionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            name: "Sentinel".to_owned(),
            scale: 3,
            always_on_top: true,
            launch_at_login: false,
            work_mode: true,
            read_titles: true,
            sleep_after_secs: 180,
            poll_ms: 3000,
            thresholds: Thresholds::default(),
            avatar: Avatar::default(),
            x: None,
            y: None,
        }
    }
}

fn valid_hex(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn colour_or(value: String, fallback: &str) -> String {
    if valid_hex(&value) {
        value.to_lowercase()
    } else {
        fallback.to_owned()
    }
}

impl CompanionSettings {
    /// Returns a copy that is safe to act on: numbers clamped, colours checked,
    /// the name trimmed and bounded, thresholds put in ascending order.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();

        self.name = self.name.trim().chars().take(24).collect();
        if self.name.is_empty() {
            self.name = defaults.name;
        }
        self.scale = self.scale.clamp(2, 12);
        self.sleep_after_secs = self.sleep_after_secs.clamp(10, 86_400);
        self.poll_ms = self.poll_ms.clamp(1_000, 15_000);

        let t = &mut self.thresholds;
        t.cpu_medium = t.cpu_medium.clamp(1, 100);
        t.cpu_high = t.cpu_high.clamp(t.cpu_medium, 100);
        t.cpu_very_high = t.cpu_very_high.clamp(t.cpu_high, 100);
        t.mem_high = t.mem_high.clamp(1, 100);
        t.mem_very_high = t.mem_very_high.clamp(t.mem_high, 100);
        t.apps_high = t.apps_high.clamp(1, 999);
        t.apps_very_high = t.apps_very_high.clamp(t.apps_high, 999);

        let a = &mut self.avatar;
        a.body = colour_or(std::mem::take(&mut a.body), &defaults.avatar.body);
        a.eyes = colour_or(std::mem::take(&mut a.eyes), &defaults.avatar.eyes);
        a.screen = colour_or(std::mem::take(&mut a.screen), &defaults.avatar.screen);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizing_repairs_untrusted_values() {
        let mut s = CompanionSettings {
            name: "   ".to_owned(),
            scale: 99,
            poll_ms: 5,
            sleep_after_secs: 0,
            ..Default::default()
        };
        s.thresholds.cpu_medium = 90;
        s.thresholds.cpu_high = 10;
        s.avatar.body = "red".to_owned();
        s.avatar.eyes = "#ABCDEF".to_owned();

        let s = s.sanitized();

        assert_eq!(s.name, "Sentinel");
        assert_eq!(s.scale, 12);
        assert_eq!(s.poll_ms, 1_000);
        assert_eq!(s.sleep_after_secs, 10);
        assert!(s.thresholds.cpu_high >= s.thresholds.cpu_medium);
        assert_eq!(s.avatar.body, Avatar::default().body);
        assert_eq!(s.avatar.eyes, "#abcdef");
    }

    #[test]
    fn a_partial_file_keeps_defaults_for_the_rest() {
        let s: CompanionSettings = serde_json::from_str(r#"{"name":"Rex"}"#).expect("parses");
        assert_eq!(s.name, "Rex");
        assert_eq!(s.scale, 3);
    }
}
