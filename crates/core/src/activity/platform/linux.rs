//! X11 backend: reads the window manager's EWMH properties over the X socket.
//!
//! Wayland hides other apps' windows from clients by design, so on a Wayland
//! session this reports nothing for apps and idle time rather than showing
//! XWayland's partial view as if it were everything.

use std::fs;

use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as _;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use super::{is_shell, Reading};
use crate::activity::Foreground;

/// Atoms looked up once per connection.
struct Atoms {
    client_list: Atom,
    active_window: Atom,
    wm_pid: Atom,
    wm_name: Atom,
    utf8: Atom,
}

struct Conn {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
}

/// The X11 connection, re-established on failure.
pub struct Backend {
    conn: Option<Conn>,
    wayland: bool,
}

type R<T> = Result<T, Box<dyn std::error::Error>>;

fn atom(conn: &RustConnection, name: &str) -> R<Atom> {
    Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

impl Backend {
    pub fn new() -> Self {
        let wayland = std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland");
        Self {
            conn: None,
            wayland,
        }
    }

    fn connect() -> R<Conn> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms {
            client_list: atom(&conn, "_NET_CLIENT_LIST")?,
            active_window: atom(&conn, "_NET_ACTIVE_WINDOW")?,
            wm_pid: atom(&conn, "_NET_WM_PID")?,
            wm_name: atom(&conn, "_NET_WM_NAME")?,
            utf8: atom(&conn, "UTF8_STRING")?,
        };
        Ok(Conn { conn, root, atoms })
    }

    /// One reading. Any X error drops the connection so the next call retries.
    pub fn read(&mut self, titles: bool) -> Reading {
        if self.wayland {
            return Reading::default();
        }
        if self.conn.is_none() {
            self.conn = Self::connect().ok();
        }
        let Some(c) = &self.conn else {
            return Reading::default();
        };
        match read_with(c, titles) {
            Ok(reading) => reading,
            Err(_) => {
                self.conn = None;
                Reading::default()
            }
        }
    }
}

fn cardinals(c: &Conn, win: Window, prop: Atom) -> R<Vec<u32>> {
    let reply = c
        .conn
        .get_property(false, win, prop, AtomEnum::ANY, 0, u32::MAX)?
        .reply()?;
    Ok(reply.value32().map(Iterator::collect).unwrap_or_default())
}

fn first(c: &Conn, win: Window, prop: Atom) -> R<Option<u32>> {
    Ok(cardinals(c, win, prop)?.first().copied())
}

fn wm_class(c: &Conn, win: Window) -> R<String> {
    let reply = c
        .conn
        .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)?
        .reply()?;
    // "instance\0class\0": the class is the second field.
    let class = reply
        .value
        .split(|b| *b == 0)
        .nth(1)
        .map(|s| String::from_utf8_lossy(s).to_lowercase())
        .unwrap_or_default();
    Ok(class)
}

fn process_name(pid: u32) -> Option<String> {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = comm.trim().to_lowercase();
    (!name.is_empty()).then_some(name)
}

fn title(c: &Conn, win: Window) -> R<Option<String>> {
    let reply = c
        .conn
        .get_property(false, win, c.atoms.wm_name, c.atoms.utf8, 0, 256)?
        .reply()?;
    Ok((!reply.value.is_empty()).then(|| String::from_utf8_lossy(&reply.value).into_owned()))
}

fn identify(c: &Conn, win: Window) -> R<Option<(String, String)>> {
    let Some(pid) = first(c, win, c.atoms.wm_pid)? else {
        return Ok(None);
    };
    let Some(name) = process_name(pid) else {
        return Ok(None);
    };
    let class = wm_class(c, win)?;
    Ok(Some((name, class)))
}

fn read_with(c: &Conn, titles: bool) -> R<Reading> {
    let windows = cardinals(c, c.root, c.atoms.client_list)?;

    let mut apps = std::collections::HashSet::new();
    for win in windows {
        let Some((name, _)) = identify(c, win)? else {
            continue;
        };
        if is_shell(&name) {
            continue;
        }
        apps.insert(name);
    }

    let active = first(c, c.root, c.atoms.active_window)?.filter(|w| *w != 0);
    let foreground = match active {
        Some(win) => match identify(c, win)? {
            Some((name, class)) => Some(Foreground {
                name,
                alt: class,
                title: if titles { title(c, win)? } else { None },
            }),
            None => None,
        },
        None => None,
    };

    let idle = c
        .conn
        .screensaver_query_info(c.root)?
        .reply()
        .ok()
        .map(|r| u64::from(r.ms_since_user_input) / 1000);

    Ok(Reading {
        foreground,
        app_count: Some(apps.len() as u32),
        idle_secs: idle,
    })
}
