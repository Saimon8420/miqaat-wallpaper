//! The notification-area icon, its menu, and prayer-time notifications.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY: u32 = WM_APP + 1;

pub const CMD_SETTINGS: u32 = 1;
pub const CMD_NEXT_SCENE: u32 = 2;
pub const CMD_PAUSE: u32 = 3;
pub const CMD_REFRESH: u32 = 4;
pub const CMD_CREDIT: u32 = 5;
pub const CMD_ABOUT: u32 = 6;
pub const CMD_EXIT: u32 = 7;

fn copy(dst: &mut [u16], s: &str) {
    let v: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..v.len()].copy_from_slice(&v);
    dst[v.len()] = 0;
}

pub fn icon(size: i32) -> HICON {
    unsafe {
        let inst = GetModuleHandleW(None).unwrap_or_default();
        LoadImageW(Some(inst.into()), PCWSTR(1 as _), IMAGE_ICON, size, size, LR_DEFAULTCOLOR).map(|h| HICON(h.0)).unwrap_or_default()
    }
}

fn data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: 1, ..Default::default() }
}

pub fn add(hwnd: HWND) {
    unsafe {
        let mut d = data(hwnd);
        d.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        d.uCallbackMessage = WM_TRAY;
        d.hIcon = icon(GetSystemMetrics(SM_CXSMICON));
        copy(&mut d.szTip, "Miqaat Wallpaper");
        let _ = Shell_NotifyIconW(NIM_ADD, &d);
    }
}

pub fn remove(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data(hwnd));
    }
}

/// Hover text, e.g. "Asr in 1h 05m".
pub fn tip(hwnd: HWND, text: &str) {
    unsafe {
        let mut d = data(hwnd);
        d.uFlags = NIF_TIP;
        copy(&mut d.szTip, &format!("Miqaat Wallpaper\n{text}"));
        let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
    }
}

/// A quiet Windows notification (no sound).
pub fn notify(hwnd: HWND, title: &str, body: &str) {
    unsafe {
        let mut d = data(hwnd);
        d.uFlags = NIF_INFO;
        copy(&mut d.szInfoTitle, title);
        copy(&mut d.szInfo, body);
        d.dwInfoFlags = NIIF_USER | NIIF_LARGE_ICON | NIIF_NOSOUND;
        d.hBalloonIcon = icon(GetSystemMetrics(SM_CXICON));
        let _ = Shell_NotifyIconW(NIM_MODIFY, &d);
    }
}

/// Shows the right-click menu and returns the chosen command (0 if none).
pub fn menu(hwnd: HWND, paused: bool, credit: &str) -> u32 {
    unsafe {
        let Ok(m) = CreatePopupMenu() else { return 0 };
        let item = |id: u32, text: &str| {
            let w: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let _ = AppendMenuW(m, MF_STRING, id as usize, PCWSTR(w.as_ptr()));
        };
        item(CMD_SETTINGS, "Settings…");
        item(CMD_NEXT_SCENE, "Next picture");
        item(CMD_PAUSE, if paused { "Resume" } else { "Pause" });
        item(CMD_REFRESH, "Refresh prayer times");
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, None);
        item(CMD_CREDIT, &format!("Photo: {credit}"));
        item(CMD_ABOUT, "About Miqaat Wallpaper");
        let _ = AppendMenuW(m, MF_SEPARATOR, 0, None);
        item(CMD_EXIT, "Exit");
        let _ = SetMenuDefaultItem(m, CMD_SETTINGS, 0);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, None, hwnd, None);
        let _ = DestroyMenu(m);
        cmd.0 as u32
    }
}

pub fn open_url(url: &str) {
    unsafe {
        let w: Vec<u16> = url.encode_utf16().chain([0]).collect();
        ShellExecuteW(None, w!("open"), PCWSTR(w.as_ptr()), None, None, SW_SHOWNORMAL);
    }
}
