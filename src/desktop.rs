//! Finding the window that sits behind the desktop icons, so the wallpaper can be drawn there.
//!
//! Sending Progman the undocumented message 0x052C asks Explorer to create a "WorkerW" window
//! between the wallpaper and the icons. Where that window ends up depends on the Windows build:
//! - Windows 11 24H2 and later: it is a child of Progman, next to the icons (SHELLDLL_DefView).
//! - Earlier builds: it is a top-level window placed right after the one that holds the icons.

use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, SendMessageTimeoutW, SMTO_NORMAL,
};

pub struct Host {
    /// The window our wallpaper becomes a child of.
    pub parent: HWND,
    /// True on 24H2+ where the host is inside Progman.
    pub modern: bool,
}

pub fn find_host() -> Option<Host> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), None).ok()?;
        let mut _r = 0usize;
        // Both parameter forms are used by different Explorer versions; sending both is harmless.
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(0x1), SMTO_NORMAL, 1000, Some(&mut _r));
        let _ = SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, Some(&mut _r));

        // 24H2+: WorkerW lives inside Progman.
        if let Ok(worker) = FindWindowExW(Some(progman), None, w!("WorkerW"), None) {
            return Some(Host { parent: worker, modern: true });
        }

        // Older builds: find the top-level window holding the icons, then the WorkerW after it.
        let mut icons_top = HWND::default();
        let _ = EnumWindows(Some(find_icons), LPARAM(&mut icons_top as *mut HWND as isize));
        if !icons_top.is_invalid() {
            if let Ok(worker) = FindWindowExW(None, Some(icons_top), w!("WorkerW"), None) {
                return Some(Host { parent: worker, modern: false });
            }
        }
        None
    }
}

unsafe extern "system" fn find_icons(top: HWND, out: LPARAM) -> BOOL {
    if FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), None).is_ok() {
        *(out.0 as *mut HWND) = top;
        return BOOL(0);
    }
    BOOL(1)
}
