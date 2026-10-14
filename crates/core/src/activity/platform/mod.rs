//! Per-OS readings of the foreground app, open apps and idle time.
//!
//! Each backend talks to the OS directly (X11 over its socket, the Win32 API);
//! none of them runs a command. Anything a backend cannot know is `None` and is
//! never guessed.

use super::Foreground;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Process names that own windows but aren't apps the user is running.
const SHELL: &[&str] = &[
    "gnome-shell",
    "plasmashell",
    "xfdesktop",
    "xfce4-panel",
    "cinnamon",
    "budgie-panel",
    "ibus-ui-gtk3",
    "nemo-desktop",
    "explorer",
    "applicationframehost",
    "textinputhost",
    "shellexperiencehost",
    "searchhost",
    "startmenuexperiencehost",
    "systemsettings",
    "lockapp",
];

/// What one probe found.
#[derive(Debug, Default)]
pub struct Reading {
    /// The app in front, if the platform can tell.
    pub foreground: Option<Foreground>,
    /// Distinct apps with a window, if the platform can tell.
    pub app_count: Option<u32>,
    /// Seconds since the last keyboard or mouse input, if the platform can tell.
    pub idle_secs: Option<u64>,
}

/// The platform backend, or a stub that reports nothing.
pub struct Probe {
    #[cfg(target_os = "linux")]
    inner: linux::Backend,
    #[cfg(windows)]
    inner: windows::Backend,
}

impl Probe {
    /// Creates the backend for this OS.
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            inner: linux::Backend::new(),
            #[cfg(windows)]
            inner: windows::Backend::new(),
        }
    }

    /// Takes one reading. `titles` asks for the foreground window title.
    #[allow(unused_variables)]
    pub fn read(&mut self, titles: bool) -> Reading {
        #[cfg(any(target_os = "linux", windows))]
        {
            let mut reading = self.inner.read(titles);
            // Shell components own windows but aren't apps the user opened.
            if let Some(fg) = &reading.foreground {
                if SHELL.contains(&fg.name.as_str()) {
                    reading.foreground = None;
                }
            }
            reading
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Reading::default()
        }
    }
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a process name is a shell component rather than a user app.
pub(crate) fn is_shell(name: &str) -> bool {
    SHELL.contains(&name)
}
