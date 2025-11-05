//! Miqaat Wallpaper: a calm live wallpaper for Windows.
//!
//! Draws a photo of a mosque or a quiet landscape behind the desktop icons, with the clock,
//! today's prayer times (Miqaat API), the next prayer, and one authentic reminder with its
//! source. It redraws about once a minute and stays around 15–35 MB of memory.
//!
//! Two windows: a hidden top-level "control" window that owns the tray icon and timers, and the
//! wallpaper window itself, a child of Explorer's desktop. If Explorer restarts or the screens
//! change, the wallpaper window is simply created again.

#![windows_subsystem = "windows"]

mod app;
mod assets;
mod content;
mod desktop;
mod hijri;
mod i18n;
mod install;
mod light;
mod render;
mod settings;
mod settings_ui;
mod sysmon;
mod times;
mod tray;

use app::App;
use settings::Settings;
use std::cell::RefCell;
use windows::core::{w, BOOL, HSTRING};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Controls::{InitCommonControlsEx, ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::WindowsAndMessaging::*;

const TIMER_MINUTE: usize = 1;
const TIMER_SECOND: usize = 2;
const TIMER_MONITOR: usize = 3;
const TIMER_ATTACH: usize = 4;
const TIMER_RETRY: usize = 5;
const WM_TIMES: u32 = WM_APP + 2;
const WM_SHOW_SETTINGS: u32 = WM_APP + 3;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static WINDOWS: RefCell<(HWND, HWND)> = const { RefCell::new((HWND(std::ptr::null_mut()), HWND(std::ptr::null_mut()))) };
}

/// Runs `f` on the app state. Never call Windows functions that send messages back to our
/// windows from inside `f` (the state would be borrowed twice).
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

/// Appends a line to %TEMP%\miqaat-wallpaper.log (problems only, so it stays tiny).
pub fn log(line: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("miqaat-wallpaper.log");
    if std::fs::metadata(&path).map(|m| m.len() > 256 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {line}", times::utc_seconds());
    }
}

fn ctl() -> HWND {
    WINDOWS.with(|w| w.borrow().0)
}

fn wall() -> HWND {
    WINDOWS.with(|w| w.borrow().1)
}

fn invalidate(r: Option<RECT>) {
    let h = wall();
    if !h.is_invalid() {
        unsafe {
            let _ = InvalidateRect(Some(h), r.as_ref().map(|r| r as *const RECT), false);
        }
    }
}

/// Starts fetching the month the city's date needs, unless it is cached or already loaded.
fn ensure_times() {
    let job = with_app(|a| {
        let key = a.wanted_key();
        if a.month.as_ref().map(|m| m.key == key).unwrap_or(false) || a.fetching {
            return None;
        }
        if let Some(m) = times::cached(&key) {
            a.month = Some(m);
            a.failed = false;
            return None;
        }
        a.fetching = true;
        let (days, _) = a.now();
        let (y, m, _) = times::civil_from_days(days);
        Some((a.s.lat, a.s.lng, a.s.method.clone(), a.s.madhab.clone(), y, m))
    })
    .flatten();
    if let Some((lat, lng, method, madhab, y, m)) = job {
        let target = ctl().0 as isize;
        std::thread::spawn(move || {
            let r = Box::new(times::fetch(lat, lng, &method, &madhab, y, m));
            let p = Box::into_raw(r) as isize;
            unsafe {
                if PostMessageW(Some(HWND(target as _)), WM_TIMES, WPARAM(0), LPARAM(p)).is_err() {
                    drop(Box::from_raw(p as *mut Result<times::Month, String>));
                }
            }
        });
    }
}

/// Called by the settings window with the changed settings.
pub fn apply_settings(n: Settings) {
    let Some(old) = with_app(|a| std::mem::replace(&mut a.s, n.clone())) else { return };
    if old == n {
        return;
    }
    n.save();
    if old.autostart != n.autostart {
        install::set_autostart(n.autostart);
    }
    if old.scene != n.scene || old.rotate != n.rotate {
        with_app(|a| a.scene_offset = 0);
    }
    if old.times_key() != n.times_key() {
        with_app(|a| {
            a.month = None;
            a.failed = false;
        });
        ensure_times();
    }
    if old.kinds != n.kinds || old.follow_time != n.follow_time || old.lang != n.lang {
        with_app(|a| a.rotate_reminder(true));
    }
    arm_timers();
    invalidate(None);
}

/// Seconds and system-monitor timers follow the settings.
fn arm_timers() {
    let (seconds, monitor, every) = with_app(|a| {
        if a.s.show_monitor && a.sysmon.is_none() {
            a.sysmon = Some(sysmon::Monitor::new());
        } else if !a.s.show_monitor {
            a.sysmon = None;
        }
        (a.s.seconds, a.s.show_monitor, a.s.monitor_every.clamp(1, 60))
    })
    .unwrap_or((false, false, 2));
    let h = ctl();
    unsafe {
        if seconds {
            SetTimer(Some(h), TIMER_SECOND, 1000, None);
        } else {
            let _ = KillTimer(Some(h), TIMER_SECOND);
        }
        if monitor {
            SetTimer(Some(h), TIMER_MONITOR, every * 1000, None);
        } else {
            let _ = KillTimer(Some(h), TIMER_MONITOR);
        }
    }
}

fn ms_to_next_minute() -> u32 {
    let s = times::utc_seconds();
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_millis()).unwrap_or(0);
    (60 - (s % 60) as u32) * 1000 - ms + 50
}

/// Is a full-screen game, video or presentation running?
fn fullscreen_busy() -> bool {
    unsafe { SHQueryUserNotificationState().map(|s| s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE).unwrap_or(false) }
}

/// The once-a-minute update: times, reminder, notifications, and the smallest repaint needed.
fn tick() {
    ensure_times();
    let pause_fs = with_app(|a| a.s.pause_fullscreen).unwrap_or(true);
    let busy = pause_fs && fullscreen_busy();
    let was_busy = with_app(|a| std::mem::replace(&mut a.busy, busy)).unwrap_or(false);
    let notes = with_app(|a| a.due_notifications()).unwrap_or_default();
    for (title, body) in notes {
        tray::notify(ctl(), &title, &body);
    }
    if busy || with_app(|a| a.paused).unwrap_or(false) {
        return;
    }
    let plan = with_app(|a| {
        a.rotate_reminder(false);
        let f = a.frame();
        let key = f.light.key();
        let scenes = a.scenes_now();
        let full = was_busy || key != a.light_key || f.zoom != 1.0 || scenes != a.scene_key;
        a.light_key = key;
        let tip = if f.next.is_empty() { String::new() } else { f.next.clone() };
        (if full { None } else { a.panel_rect() }, tip)
    });
    if let Some((rect, tip)) = plan {
        tray::tip(ctl(), &tip);
        invalidate(rect);
    }
}

/// Every monitor's rectangle in `parent`'s client coordinates, and which one is primary.
fn monitors(parent: HWND) -> (Vec<RECT>, usize) {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(m, &mut info).as_bool() {
            (*(out.0 as *mut Vec<(RECT, bool)>)).push((info.rcMonitor, info.dwFlags & 1 == 1));
        }
        BOOL(1)
    }
    let mut found: Vec<(RECT, bool)> = vec![];
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    let to_client = |r: RECT| {
        let mut pts = [POINT { x: r.left, y: r.top }, POINT { x: r.right, y: r.bottom }];
        unsafe { MapWindowPoints(None, Some(parent), &mut pts) };
        RECT { left: pts[0].x, top: pts[0].y, right: pts[1].x, bottom: pts[1].y }
    };
    let all: Vec<RECT> = found.iter().map(|(r, _)| to_client(*r)).collect();
    let primary = found.iter().position(|(_, p)| *p).unwrap_or(0);
    (all, primary)
}

/// Creates the wallpaper window behind the icons (again, after Explorer or screens change).
fn attach() {
    let old = wall();
    unsafe {
        if !old.is_invalid() {
            WINDOWS.with(|w| w.borrow_mut().1 = HWND::default());
            let _ = DestroyWindow(old);
        }
        let Some(host) = desktop::find_host() else {
            // Explorer isn't ready yet (e.g. right after sign-in): try again shortly.
            log("desktop host not found; retrying");
            SetTimer(Some(ctl()), TIMER_ATTACH, 3000, None);
            return;
        };
        let mut rc = RECT::default();
        let _ = GetClientRect(host.parent, &mut rc);
        let inst = GetModuleHandleW(None).unwrap_or_default();
        // 24H2+ composes the desktop without a GDI surface; a layered child gets its own one.
        let hwnd = CreateWindowExW(
            if host.modern { WS_EX_LAYERED } else { WINDOW_EX_STYLE::default() },
            w!("MiqaatWallpaper"),
            w!("Miqaat Wallpaper"),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            rc.right - rc.left,
            rc.bottom - rc.top,
            Some(host.parent),
            None,
            Some(inst.into()),
            None,
        )
        .unwrap_or_else(|e| {
            log(&format!("wallpaper window: {e}"));
            HWND::default()
        });
        if host.modern {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
        }
        WINDOWS.with(|w| w.borrow_mut().1 = hwnd);
        let (mons, primary) = monitors(host.parent);
        with_app(|a| {
            a.monitors = mons;
            a.primary = primary;
        });
    }
    invalidate(None);
}

/// Puts the normal wallpaper back by asking Windows to redraw it.
fn restore_wallpaper() {
    unsafe {
        let mut buf = [0u16; 520];
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr() as _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr() as _), SPIF_SENDCHANGE);
        }
    }
}

fn user_wallpaper() -> String {
    let mut buf = [0u16; 520];
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr() as _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&buf[..n])
}

fn quit() {
    let w = wall();
    unsafe {
        WINDOWS.with(|x| x.borrow_mut().1 = HWND::default());
        if !w.is_invalid() {
            let _ = DestroyWindow(w);
        }
        restore_wallpaper();
        tray::remove(ctl());
        let _ = DestroyWindow(ctl());
    }
}

unsafe extern "system" fn wall_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            with_app(|a| {
                a.ensure_renderer();
                let f = a.frame();
                let st = a.style();
                match &a.renderer {
                    Some(r) => {
                        if let Err(e) = r.paint(ps.hdc, ps.rcPaint, &a.monitors, a.primary, &f, &st) {
                            log(&format!("paint: {e}"));
                        }
                    }
                    None => log("no renderer"),
                }
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            // Explorer took the desktop away (restart or screen change): come back shortly.
            if wall() == hwnd {
                WINDOWS.with(|w| w.borrow_mut().1 = HWND::default());
                SetTimer(Some(ctl()), TIMER_ATTACH, 2000, None);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

static mut TASKBAR_CREATED: u32 = 0;

unsafe extern "system" fn ctl_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == TASKBAR_CREATED && msg != 0 {
        // Explorer restarted: the tray icon and the desktop window are gone.
        tray::add(hwnd);
        SetTimer(Some(hwnd), TIMER_ATTACH, 2000, None);
        return LRESULT(0);
    }
    match msg {
        WM_TIMER => {
            match wp.0 {
                TIMER_MINUTE => {
                    SetTimer(Some(hwnd), TIMER_MINUTE, ms_to_next_minute(), None);
                    tick();
                }
                TIMER_SECOND => {
                    if with_app(|a| !a.busy && !a.paused).unwrap_or(false) {
                        let r = with_app(|a| a.renderer.as_ref().map(|r| r.clock_rect.get())).flatten();
                        invalidate(r);
                    }
                }
                TIMER_MONITOR => {
                    let r = with_app(|a| {
                        if a.busy || a.paused || (a.s.battery_saver && app::on_battery()) {
                            return None;
                        }
                        a.sysmon.as_mut()?.sample();
                        a.renderer.as_ref().map(|r| r.strip_rect.get())
                    })
                    .flatten();
                    if let Some(r) = r {
                        if r.right > r.left {
                            invalidate(Some(r));
                        } else {
                            invalidate(None);
                        }
                    }
                }
                TIMER_ATTACH => {
                    let _ = KillTimer(Some(hwnd), TIMER_ATTACH);
                    attach();
                }
                TIMER_RETRY => {
                    let _ = KillTimer(Some(hwnd), TIMER_RETRY);
                    ensure_times();
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TIMES => {
            let r = *Box::from_raw(lp.0 as *mut Result<times::Month, String>);
            let ok = r.is_ok();
            with_app(|a| {
                a.fetching = false;
                match r {
                    Ok(m) => {
                        a.month = Some(m);
                        a.failed = false;
                        a.rotate_reminder(true);
                    }
                    Err(_) => a.failed = a.month.is_none(),
                }
            });
            if !ok {
                SetTimer(Some(hwnd), TIMER_RETRY, 10 * 60 * 1000, None);
            }
            invalidate(None);
            LRESULT(0)
        }
        tray::WM_TRAY => {
            match (lp.0 & 0xFFFF) as u32 {
                WM_LBUTTONDBLCLK => settings_ui::open(),
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    let (paused, credit, source) = with_app(|a| {
                        let s = &assets::SCENES[a.scene_key.get(a.primary).copied().unwrap_or(0)];
                        let custom = a.s.scene == "custom";
                        (a.paused, if custom { "your own photo".to_string() } else { format!("{} · {}", s.name, s.credit) }, if custom { String::new() } else { s.source.to_string() })
                    })
                    .unwrap_or((false, String::new(), String::new()));
                    match tray::menu(hwnd, paused, &credit) {
                        tray::CMD_SETTINGS => settings_ui::open(),
                        tray::CMD_NEXT_SCENE => {
                            with_app(|a| {
                                if a.s.scene == "custom" || a.s.rotate == "keep" {
                                    let n = assets::SCENES.len();
                                    let i = assets::SCENES.iter().position(|x| x.id == a.s.scene).map(|i| (i + 1) % n).unwrap_or(0);
                                    a.s.scene = assets::SCENES[i].id.into();
                                    a.s.save();
                                } else {
                                    a.scene_offset += 1;
                                }
                            });
                            invalidate(None);
                        }
                        tray::CMD_PAUSE => {
                            with_app(|a| a.paused = !a.paused);
                            invalidate(None);
                        }
                        tray::CMD_REFRESH => {
                            with_app(|a| {
                                if let Some(m) = &a.month {
                                    let _ = std::fs::remove_file(settings::data_dir().join(format!("times-{}.json", m.key)));
                                }
                                a.month = None;
                            });
                            ensure_times();
                        }
                        tray::CMD_CREDIT if !source.is_empty() => tray::open_url(&source),
                        tray::CMD_ABOUT => tray::open_url(install::REPO),
                        tray::CMD_EXIT => quit(),
                        _ => {}
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_SHOW_SETTINGS => {
            settings_ui::open();
            LRESULT(0)
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            SetTimer(Some(hwnd), TIMER_ATTACH, 1500, None);
            LRESULT(0)
        }
        WM_SETTINGCHANGE if wp.0 == SPI_SETWORKAREA.0 as usize => {
            SetTimer(Some(hwnd), TIMER_ATTACH, 1500, None);
            LRESULT(0)
        }
        WM_TIMECHANGE | WM_POWERBROADCAST => {
            tick();
            invalidate(None);
            LRESULT(1)
        }
        WM_CLOSE => {
            quit();
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// `--set key=value` (repeatable) overrides settings for one run, e.g. for screenshots.
fn overrides(mut s: Settings, args: &[String]) -> Settings {
    for pair in args.windows(2).filter(|p| p[0] == "--set").map(|p| p[1].clone()) {
        if let Some((k, v)) = pair.split_once('=') {
            let mut o = serde_json::to_value(&s).unwrap_or_default();
            o[k] = serde_json::from_str(v).unwrap_or(serde_json::Value::String(v.into()));
            s = serde_json::from_value(o).unwrap_or(s);
        }
    }
    s
}

fn arg_after<'a>(args: &'a [String], name: &str) -> Option<&'a String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1))
}

/// `--snapshot out.png [--size 1920x1080] [--at 17:30] [--set k=v …]`: draws one frame to a
/// PNG and exits. Used for the README pictures and to check changes without the desktop.
fn snapshot(args: &[String], out: &str) -> windows::core::Result<()> {
    let s = overrides(Settings::load(), args);
    let (w, h) = arg_after(args, "--size").and_then(|v| v.split_once('x')).and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or((1920, 1080));
    let mut a = App::new(s);
    a.monitors = vec![RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }];
    let key = a.wanted_key();
    a.month = times::cached(&key).or_else(|| {
        let (days, _) = a.now();
        let (y, m, _) = times::civil_from_days(days);
        times::fetch(a.s.lat, a.s.lng, &a.s.method, &a.s.madhab, y, m).ok()
    });
    if a.s.show_monitor {
        let mut m = sysmon::Monitor::new();
        for _ in 0..12 {
            std::thread::sleep(std::time::Duration::from_millis(150));
            m.sample();
        }
        a.sysmon = Some(m);
    }
    // `--at 17:30`: show another hour of today (clock, light, next prayer, reminder).
    a.fake_now = arg_after(args, "--at").and_then(|v| v.split_once(':')).and_then(|(h, m)| Some(h.parse::<u32>().ok()? * 3600 + m.parse::<u32>().ok()? * 60));
    a.rotate_reminder(true);
    a.ensure_renderer();
    let f = a.frame();
    a.renderer.as_ref().ok_or_else(|| windows::core::Error::new(E_FAIL, "renderer"))?.snapshot(out, w, h, &f, &a.style())
}

fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_STANDARD_CLASSES | ICC_BAR_CLASSES };
        let _ = InitCommonControlsEx(&icc);
    }

    if let Some(out) = arg_after(&args, "--snapshot") {
        return snapshot(&args, out);
    }
    if has("--uninstall") {
        install::uninstall();
        return Ok(());
    }
    if has("--set-autostart") {
        install::set_autostart(true);
        return Ok(());
    }
    // Run from a download: offer to install (No = just run this once from here).
    if !has("--portable") && !install::is_installed_copy() && install::install() {
        return Ok(());
    }

    unsafe {
        // One copy at a time; starting it again opens the settings of the running one.
        let _mutex = CreateMutexW(None, false, w!("Local\\MiqaatWallpaper.Single"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(h) = FindWindowW(w!("MiqaatWallpaperCtl"), None) {
                let _ = PostMessageW(Some(h), WM_SHOW_SETTINGS, WPARAM(0), LPARAM(0));
            }
            return Ok(());
        }

        let s = overrides(Settings::load(), &args);
        if !std::path::Path::new(&settings::data_dir().join("settings.json")).exists() {
            s.save();
        }
        let mut a = App::new(s);
        a.user_wallpaper = user_wallpaper();
        a.rotate_reminder(true);
        APP.with(|x| *x.borrow_mut() = Some(a));

        let inst = GetModuleHandleW(None)?;
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wall_proc), hInstance: inst.into(), lpszClassName: w!("MiqaatWallpaper"), ..Default::default() });
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(ctl_proc), hInstance: inst.into(), lpszClassName: w!("MiqaatWallpaperCtl"), ..Default::default() });
        TASKBAR_CREATED = RegisterWindowMessageW(w!("TaskbarCreated"));
        let ctl = CreateWindowExW(WS_EX_TOOLWINDOW, w!("MiqaatWallpaperCtl"), w!("Miqaat Wallpaper"), WS_POPUP, 0, 0, 0, 0, None, None, Some(inst.into()), None)?;
        WINDOWS.with(|w| w.borrow_mut().0 = ctl);
        tray::add(ctl);

        ensure_times();
        attach();
        arm_timers();
        SetTimer(Some(ctl), TIMER_MINUTE, ms_to_next_minute(), None);
        if has("--welcome") {
            tray::notify(ctl, "Miqaat Wallpaper is on your desktop", "Right-click the moon icon in the taskbar corner for settings.");
            settings_ui::open();
        } else if has("--updated") {
            tray::notify(ctl, &format!("Miqaat Wallpaper {}", install::VERSION), "Updated. Your settings were kept.");
        } else if let Some(i) = args.iter().position(|a| a == "--settings") {
            settings_ui::open_at(args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0));
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if settings_ui::dialog_message(&msg) {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = HSTRING::new();
    }
    Ok(())
}
