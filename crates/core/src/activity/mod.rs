//! What the user is doing right now, judged from the foreground app.
//!
//! The platform probes (`platform`) report a foreground app name, optionally a
//! window title, how many apps are open and how long the user has been idle.
//! Everything here is pure: given those facts, decide an [`Activity`]. Window
//! titles are read only when the caller asks, matched in memory against keyword
//! patterns and dropped; only the resulting category leaves this module.
//!
//! No command is ever run to get this data. Linux talks to the X server over
//! its socket and Windows calls the Win32 API directly.

use serde::Serialize;

use crate::settings::companion::CompanionSettings;
use crate::system::SystemMonitor;

pub mod platform;

/// What the foreground app is being used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    /// A web browser.
    Browser,
    /// A code editor or IDE.
    Dev,
    /// A terminal.
    Terminal,
    /// Video playback.
    Video,
    /// A game or game launcher.
    Game,
    /// Music playback.
    Music,
    /// Messaging, mail or calls.
    Chat,
    /// Design, video or audio editing.
    Design,
    /// Documents and reading.
    Office,
}

impl Activity {
    /// Media activities make the companion relaxed rather than "working".
    pub fn is_media(self) -> bool {
        matches!(self, Self::Video | Self::Music)
    }

    /// The lowercase name used in storage and on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Dev => "dev",
            Self::Terminal => "terminal",
            Self::Video => "video",
            Self::Game => "game",
            Self::Music => "music",
            Self::Chat => "chat",
            Self::Design => "design",
            Self::Office => "office",
        }
    }
}

/// The foreground app as the probes report it.
#[derive(Debug, Clone, Default)]
pub struct Foreground {
    /// Lowercase process name, without extension.
    pub name: String,
    /// Lowercase window class or extra identity (may equal `name`).
    pub alt: String,
    /// Window title, present only when title reading is on.
    pub title: Option<String>,
}

const BROWSERS: &[&str] = &[
    "chrome",
    "chromium",
    "brave",
    "firefox",
    "msedge",
    "edge",
    "opera",
    "vivaldi",
    "librewolf",
    "zen",
];
const EDITORS: &[&str] = &[
    "code",
    "codium",
    "cursor",
    "idea",
    "pycharm",
    "webstorm",
    "clion",
    "goland",
    "rider",
    "zed",
    "subl",
    "sublime_text",
    "devenv",
];
const TERMINALS: &[&str] = &[
    "terminal",
    "konsole",
    "alacritty",
    "kitty",
    "xterm",
    "tilix",
    "terminator",
    "wezterm",
    "foot",
    "windowsterminal",
    "powershell",
    "pwsh",
    "cmd",
];
const VIDEO: &[&str] = &[
    "vlc",
    "mpv",
    "celluloid",
    "totem",
    "smplayer",
    "haruna",
    "kodi",
    "stremio",
    "plex",
];
const GAMES: &[&str] = &[
    "steam",
    "lutris",
    "heroic",
    "minecraft",
    "minecraft-launcher",
    "retroarch",
    "yuzu",
    "dolphin-emu",
    "bottles",
    "supertuxkart",
    "wine",
];
const MUSIC: &[&str] = &[
    "spotify",
    "rhythmbox",
    "audacious",
    "clementine",
    "strawberry",
    "lollypop",
    "amberol",
    "youtube-music",
];
const CHAT: &[&str] = &[
    "discord",
    "telegram",
    "telegram-desktop",
    "signal",
    "signal-desktop",
    "slack",
    "whatsapp",
    "zoom",
    "teams",
    "element",
    "thunderbird",
    "skype",
];
const DESIGN: &[&str] = &[
    "gimp",
    "blender",
    "inkscape",
    "krita",
    "figma",
    "darktable",
    "kdenlive",
    "obs",
    "audacity",
    "godot",
    "aseprite",
];
const OFFICE: &[&str] = &[
    "libreoffice",
    "soffice",
    "onlyoffice",
    "wps",
    "evince",
    "okular",
    "obsidian",
    "notion",
    "logseq",
];

/// Browser tab-title keywords, checked in order; the first match wins.
const TAB_KEYWORDS: &[(Activity, &[&str])] = &[
    (
        Activity::Video,
        &[
            "youtube",
            "netflix",
            "hotstar",
            "prime video",
            "twitch",
            "vimeo",
            "disney+",
            "crunchyroll",
            "hulu",
        ],
    ),
    (
        Activity::Music,
        &[
            "spotify",
            "soundcloud",
            "youtube music",
            "apple music",
            "jiosaavn",
            "gaana",
        ],
    ),
    (
        Activity::Chat,
        &[
            "whatsapp",
            "discord",
            "telegram",
            "messenger",
            "slack",
            "gmail",
            "outlook",
            "linkedin",
            "instagram",
            "twitter",
            "reddit",
        ],
    ),
    (
        Activity::Dev,
        &[
            "github",
            "gitlab",
            "stack overflow",
            "stackoverflow",
            "localhost",
            "leetcode",
            "codepen",
            "codesandbox",
            "replit",
            "mdn web docs",
            "npmjs",
            "vercel",
            "netlify",
        ],
    ),
    (Activity::Design, &["figma", "canva", "dribbble", "behance"]),
    (
        Activity::Office,
        &[
            "google docs",
            "google sheets",
            "google slides",
            "notion",
            "overleaf",
        ],
    ),
    (
        Activity::Game,
        &[
            "chess.com",
            "lichess",
            "itch.io",
            "poki",
            "crazygames",
            "steam community",
        ],
    ),
];

fn tokens(fg: &Foreground) -> Vec<String> {
    format!("{} {}", fg.name, fg.alt)
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn has(tokens: &[String], list: &[&str]) -> bool {
    list.iter().any(|k| tokens.iter().any(|t| t == k))
}

/// Maps the foreground app (and, for browsers, its tab title) to an activity.
///
/// Returns `None` for anything unrecognised: guessing would mislabel the user.
pub fn categorize(fg: &Foreground) -> Option<Activity> {
    let t = tokens(fg);
    if t.iter().any(|x| x.starts_with("steam_app")) {
        return Some(Activity::Game);
    }
    let media_and_misc = [
        (VIDEO, Activity::Video),
        (GAMES, Activity::Game),
        (MUSIC, Activity::Music),
        (CHAT, Activity::Chat),
        (DESIGN, Activity::Design),
        (OFFICE, Activity::Office),
    ];
    for (list, activity) in media_and_misc {
        if has(&t, list) {
            return Some(activity);
        }
    }
    if has(&t, EDITORS) {
        return Some(Activity::Dev);
    }
    if has(&t, TERMINALS) {
        return Some(Activity::Terminal);
    }
    if has(&t, BROWSERS) {
        if let Some(title) = &fg.title {
            let title = title.to_lowercase();
            for (activity, words) in TAB_KEYWORDS {
                if words.iter().any(|w| title.contains(w)) {
                    return Some(*activity);
                }
            }
        }
        return Some(Activity::Browser);
    }
    None
}

/// Today's date in the user's local time zone, `YYYY-MM-DD`: the key usage is
/// grouped under, so "today" means the user's today.
pub fn local_day() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// The local date `days` days ago, `YYYY-MM-DD`.
pub fn local_day_minus(days: u32) -> String {
    (chrono::Local::now() - chrono::Duration::days(i64::from(days)))
        .format("%Y-%m-%d")
        .to_string()
}

/// One reading, sent to the companion and the usage recorder.
///
/// Never carries a window title: only the classified category and the process
/// name of the foreground app.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// CPU load, 0 to 100.
    pub cpu: f32,
    /// Memory in use, 0 to 100.
    pub mem: f32,
    /// Distinct apps open; `None` where the platform can't say.
    pub apps: Option<u32>,
    /// Seconds since last input; `None` where the platform can't say.
    pub idle_sec: Option<u64>,
    /// Foreground work activity (not media).
    pub work: Option<Activity>,
    /// True while watching video or listening to music.
    pub relaxed: bool,
    /// Any classified foreground activity.
    pub activity: Option<Activity>,
    /// Foreground process name.
    pub app: Option<String>,
}

/// Takes readings of the machine and the foreground app.
pub struct Sampler {
    monitor: SystemMonitor,
    probe: platform::Probe,
}

impl Sampler {
    /// A sampler with its own CPU baseline, independent of the dashboard's.
    pub fn new() -> Self {
        Self {
            monitor: SystemMonitor::new(),
            probe: platform::Probe::new(),
        }
    }

    /// Reads everything the companion reacts to, honouring its privacy options.
    pub fn sample(&mut self, cfg: &CompanionSettings) -> Sample {
        let info = self.monitor.snapshot();
        let reading = self.probe.read(cfg.work_mode && cfg.read_titles);

        let fg = if cfg.work_mode {
            reading.foreground
        } else {
            None
        };
        let activity = fg.as_ref().and_then(categorize);
        let media = activity.is_some_and(Activity::is_media);

        Sample {
            cpu: info.cpu.usage_percent,
            mem: if info.memory.total_bytes == 0 {
                0.0
            } else {
                (info.memory.used_bytes as f64 / info.memory.total_bytes as f64 * 100.0) as f32
            },
            apps: reading.app_count,
            idle_sec: reading.idle_secs,
            work: activity.filter(|a| !a.is_media()),
            relaxed: media,
            activity,
            app: fg.map(|f| f.name),
        }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fg(name: &str, title: Option<&str>) -> Foreground {
        Foreground {
            name: name.to_owned(),
            alt: name.to_owned(),
            title: title.map(str::to_owned),
        }
    }

    #[test]
    fn apps_map_to_activities() {
        assert_eq!(categorize(&fg("code", None)), Some(Activity::Dev));
        assert_eq!(categorize(&fg("alacritty", None)), Some(Activity::Terminal));
        assert_eq!(categorize(&fg("vlc", None)), Some(Activity::Video));
        assert_eq!(categorize(&fg("steam_app_620", None)), Some(Activity::Game));
        assert_eq!(categorize(&fg("some-unknown-app", None)), None);
    }

    #[test]
    fn browser_tabs_use_the_title_only_when_given() {
        assert_eq!(categorize(&fg("firefox", None)), Some(Activity::Browser));
        assert_eq!(
            categorize(&fg("firefox", Some("Lo-fi - YouTube"))),
            Some(Activity::Video)
        );
        assert_eq!(
            categorize(&fg("chrome", Some("repo · GitHub"))),
            Some(Activity::Dev)
        );
        // A title on a non-browser is ignored.
        assert_eq!(
            categorize(&fg("code", Some("YouTube"))),
            Some(Activity::Dev)
        );
    }
}
