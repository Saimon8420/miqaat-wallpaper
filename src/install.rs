//! Installing, updating, uninstalling and "start with Windows", all per user (no admin).
//!
//! The downloaded exe is its own installer: run from anywhere else, it offers to copy itself
//! to %LOCALAPPDATA%\Programs\Miqaat Wallpaper, adds a Start menu shortcut and an entry in
//! Settings › Apps, then starts the installed copy.

use std::path::PathBuf;
use windows::core::{w, Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "https://github.com/Saimon8420/miqaat-wallpaper";
const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const UNINSTALL_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\MiqaatWallpaper");
const VALUE: PCWSTR = w!("MiqaatWallpaper");

pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Programs").join("Miqaat Wallpaper")
}

pub fn installed_exe() -> PathBuf {
    install_dir().join("MiqaatWallpaper.exe")
}

fn shortcut() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    base.join("Microsoft\\Windows\\Start Menu\\Programs\\Miqaat Wallpaper.lnk")
}

fn same(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn is_installed_copy() -> bool {
    std::env::current_exe().map(|e| same(&e, &installed_exe())).unwrap_or(false)
}

/// `--silent`: answer every question with the default "yes" (scripted installs).
fn silent() -> bool {
    std::env::args().any(|a| a == "--silent")
}

fn ask(text: &str, flags: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    if silent() {
        return if (flags.0 & 0xF) == MB_OK.0 { IDOK } else { IDYES };
    }
    unsafe { MessageBoxW(None, &HSTRING::from(text), w!("Miqaat Wallpaper"), flags | MB_SETFOREGROUND) }
}

fn set_string(key: HKEY, name: &str, value: &str) {
    let v: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let bytes = unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) };
    unsafe {
        let _ = RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(bytes));
    }
}

fn set_dword(key: HKEY, name: &str, value: u32) {
    unsafe {
        let _ = RegSetValueExW(key, &HSTRING::from(name), None, REG_DWORD, Some(&value.to_le_bytes()));
    }
}

/// Adds or removes the "start with Windows" entry for this exe.
pub fn set_autostart(on: bool) {
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, None, REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None).is_ok() {
            if on {
                let exe = std::env::current_exe().unwrap_or_default();
                set_string(key, "MiqaatWallpaper", &format!("\"{}\"", exe.display()));
            } else {
                let _ = RegDeleteValueW(key, VALUE);
            }
            let _ = RegCloseKey(key);
        }
    }
}

/// Asks a running copy to quit and waits for it (up to 5 seconds).
fn close_running() {
    unsafe {
        for _ in 0..50 {
            match FindWindowW(w!("MiqaatWallpaperCtl"), None) {
                Ok(h) if !h.is_invalid() => {
                    let _ = PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0));
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                _ => return,
            }
        }
    }
}

fn make_shortcut(exe: &std::path::Path) -> windows::core::Result<()> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))?;
        link.SetDescription(w!("A calm prayer-time wallpaper"))?;
        link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
        link.cast::<IPersistFile>()?.Save(&HSTRING::from(shortcut().as_os_str()), true)?;
    }
    Ok(())
}

/// Copies this exe into place, registers it, and starts it. Returns true if installed.
pub fn install() -> bool {
    let Ok(me) = std::env::current_exe() else { return false };
    let dir = install_dir();
    let exe = installed_exe();
    let updating = exe.exists();
    let text = if updating {
        format!("Update Miqaat Wallpaper to version {VERSION}?\n\nYour settings are kept.")
    } else {
        format!(
            "Install Miqaat Wallpaper {VERSION}?\n\nIt installs for you only (no admin needed), starts with Windows, and puts prayer times and a daily reminder on your desktop.\n\nChoose No to just try it this once."
        )
    };
    match ask(&text, MB_YESNOCANCEL | MB_ICONQUESTION) {
        IDYES => {}
        IDNO => return false,
        _ => std::process::exit(0),
    }
    close_running();
    let _ = std::fs::create_dir_all(&dir);
    if std::fs::copy(&me, &exe).is_err() {
        ask("Could not copy the app into place. Please close Miqaat Wallpaper and try again.", MB_OK | MB_ICONWARNING);
        std::process::exit(1);
    }
    let _ = make_shortcut(&exe);
    unsafe {
        let mut key = HKEY::default();
        if RegCreateKeyExW(HKEY_CURRENT_USER, UNINSTALL_KEY, None, None, REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None).is_ok() {
            let e = exe.display().to_string();
            set_string(key, "DisplayName", "Miqaat Wallpaper");
            set_string(key, "DisplayVersion", VERSION);
            set_string(key, "Publisher", "Saimon8420");
            set_string(key, "DisplayIcon", &e);
            set_string(key, "InstallLocation", &dir.display().to_string());
            set_string(key, "UninstallString", &format!("\"{e}\" --uninstall"));
            set_string(key, "URLInfoAbout", REPO);
            set_dword(key, "NoModify", 1);
            set_dword(key, "NoRepair", 1);
            set_dword(key, "EstimatedSize", (std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0) / 1024) as u32);
            let _ = RegCloseKey(key);
        }
    }
    // Autostart follows the saved setting (on by default).
    let s = crate::settings::Settings::load();
    if s.autostart {
        let _ = std::process::Command::new(&exe).arg("--set-autostart").status();
    }
    let _ = std::process::Command::new(&exe).arg(if updating { "--updated" } else { "--welcome" }).spawn();
    true
}

/// Removes everything `install` added, plus settings, after asking.
pub fn uninstall() {
    if ask("Remove Miqaat Wallpaper from this PC?\n\nYour normal wallpaper comes back.", MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    close_running();
    set_autostart(false);
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, UNINSTALL_KEY);
    }
    let _ = std::fs::remove_file(shortcut());
    let _ = std::fs::remove_dir_all(crate::settings::data_dir());
    // A running exe can't delete itself: let a hidden shell do it a moment after we exit.
    let dir = install_dir();
    if dir.exists() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd.exe")
            .raw_arg(format!("/c ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display()))
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn();
    }
    ask("Miqaat Wallpaper has been removed.", MB_OK | MB_ICONINFORMATION);
}

