//! Win32 backend: foreground window, open top-level windows and idle time,
//! all through direct API calls (no PowerShell, no subprocess).
//!
//! Not yet exercised on a real Windows 11 machine; it is compile-checked only.

use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

use super::{is_shell, Reading};
use crate::activity::Foreground;

/// Stateless: every call asks Windows afresh.
pub struct Backend;

impl Backend {
    pub fn new() -> Self {
        Self
    }

    pub fn read(&mut self, titles: bool) -> Reading {
        // SAFETY: every call below takes valid out-pointers to local variables
        // and handles returned by the OS; handles opened here are closed here.
        unsafe {
            let mut names: HashSet<String> = HashSet::new();
            let ptr: *mut HashSet<String> = &mut names;
            EnumWindows(Some(collect), ptr as LPARAM);

            let fg_hwnd = GetForegroundWindow();
            let foreground = if fg_hwnd.is_null() {
                None
            } else {
                process_of(fg_hwnd).map(|name| Foreground {
                    alt: name.clone(),
                    name,
                    title: if titles { window_title(fg_hwnd) } else { None },
                })
            };

            Reading {
                foreground,
                app_count: Some(names.len() as u32),
                idle_secs: idle_secs(),
            }
        }
    }
}

/// `EnumWindows` callback: records the app of each real, visible top-level window.
unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `*mut HashSet<String>` passed by `read`, alive for the call.
    let names = &mut *(lparam as *mut HashSet<String>);

    if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
        return 1;
    }
    if !GetWindow(hwnd, GW_OWNER).is_null() {
        return 1;
    }
    if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 {
        return 1;
    }
    // Windows on another virtual desktop, and suspended UWP apps, are "cloaked".
    let mut cloaked: u32 = 0;
    DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED as u32,
        &mut cloaked as *mut u32 as *mut c_void,
        std::mem::size_of::<u32>() as u32,
    );
    if cloaked != 0 {
        return 1;
    }
    if window_title(hwnd).is_none() {
        return 1;
    }
    if let Some(name) = process_of(hwnd) {
        if !is_shell(&name) {
            names.insert(name);
        }
    }
    1
}

unsafe fn window_title(hwnd: HWND) -> Option<String> {
    let mut buf = [0u16; 256];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    (len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Lowercase executable name without extension, e.g. `code` for `Code.exe`.
unsafe fn process_of(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 {
        return None;
    }
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 520];
    let mut len = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
    CloseHandle(handle);
    if ok == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
}

unsafe fn idle_secs() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    if GetLastInputInfo(&mut info) == 0 {
        return None;
    }
    // Both are 32-bit millisecond tick counts; wrapping subtraction handles rollover.
    Some(u64::from(GetTickCount().wrapping_sub(info.dwTime)) / 1000)
}
