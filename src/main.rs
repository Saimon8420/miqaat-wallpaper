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
mod calligraphy;
mod extras;
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
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

const TIMER_MINUTE: usize = 1;
const TIMER_SECOND: usize = 2;
const TIMER_MONITOR: usize = 3;
const TIMER_ATTACH: usize = 4;
const TIMER_RETRY: usize = 5;
const TIMER_PEEK: usize = 6;
const WM_TIMES: u32 = WM_APP + 2;
const WM_SHOW_SETTINGS: u32 = WM_APP + 3;
const WM_EVENTS: u32 = WM_APP + 4;
const WM_UPDATE: u32 = WM_APP + 5;

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

/// After a big repaint, hands the pages it touched back to Windows. They are paged back in only
/// if used again, so Task Manager shows what the wallpaper really needs between repaints.
pub fn trim_memory() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}

/// After something big is freed (the Settings window): gives the heaps' free space back to
/// Windows, then trims.
pub fn release_memory() {
    use windows::Win32::System::Memory::{GetProcessHeaps, HeapCompact, HEAP_NONE};
    unsafe {
        let mut heaps = [HANDLE::default(); 64];
        let n = (GetProcessHeaps(&mut heaps) as usize).min(heaps.len());
        for h in &heaps[..n] {
            let _ = HeapCompact(*h, HEAP_NONE);
        }
    }
    trim_memory();
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

/// What a background download brings back: the month now (if it was needed) and its neighbours.
struct Fetched {
    current: Option<Result<times::Month, String>>,
    near: Vec<times::Month>,
    near_key: String,
}

/// Makes sure the city's current month of prayer times is loaded, plus the months before and
/// after it (for the Hijri adjustment near a month's edge). Saved months are used first; anything
/// missing is downloaded on a worker thread.
fn ensure_times() {
    let job = with_app(|a| {
        if a.fetching {
            return None;
        }
        let key = a.wanted_key();
        let (days, _) = a.now();
        let (y, m, _) = times::civil_from_days(days);
        let (s_lat, s_lng, method, madhab) = (a.s.lat, a.s.lng, a.s.method.clone(), a.s.madhab.clone());
        let mk = |(yy, mm): (i32, u32)| times::month_key(s_lat, s_lng, &method, &madhab, yy, mm);
        let near_keys: Vec<String> = times::neighbours(y, m).iter().map(|&n| mk(n)).collect();
        let mut keep = near_keys.clone();
        keep.push(key.clone());

        let need_current = !a.month.as_ref().map(|mo| mo.key == key).unwrap_or(false);
        if need_current {
            if let Some(mo) = times::cached(&key) {
                a.month = Some(mo);
                a.failed = false;
            }
        }
        let need_current = !a.month.as_ref().map(|mo| mo.key == key).unwrap_or(false);
        if a.near_key != key {
            a.near = near_keys.iter().filter_map(|k| times::cached(k)).collect();
            if a.near.len() == near_keys.len() {
                a.near_key = key.clone();
                times::prune(&keep);
            }
        }
        // Neighbours are a nice-to-have: offline, try them again at most every 10 minutes.
        let need_near = a.near_key != key && times::utc_seconds() - a.near_retry >= 600;
        if !need_current && !need_near {
            return None;
        }
        a.fetching = true;
        if need_near {
            a.near_retry = times::utc_seconds();
        }
        let missing: Vec<(i32, u32)> = if need_near { times::neighbours(y, m).into_iter().filter(|&n| !a.near.iter().any(|x| x.key == mk(n))).collect() } else { vec![] };
        Some((s_lat, s_lng, method.clone(), madhab.clone(), y, m, need_current, missing, key, keep))
    })
    .flatten();
    if let Some((lat, lng, method, madhab, y, m, need_current, missing, key, keep)) = job {
        let target = ctl().0 as isize;
        std::thread::spawn(move || {
            let current = need_current.then(|| times::fetch(lat, lng, &method, &madhab, y, m));
            let mut near = vec![];
            for (yy, mm) in missing {
                if let Ok(mo) = times::fetch(lat, lng, &method, &madhab, yy, mm) {
                    near.push(mo);
                }
            }
            times::prune(&keep);
            let p = Box::into_raw(Box::new(Fetched { current, near, near_key: key })) as isize;
            unsafe {
                if PostMessageW(Some(HWND(target as _)), WM_TIMES, WPARAM(0), LPARAM(p)).is_err() {
                    drop(Box::from_raw(p as *mut Fetched));
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
            a.near.clear();
            a.near_key.clear();
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
        (a.s.seconds && a.s.show_clock, a.s.show_monitor, a.s.monitor_every.clamp(1, 60))
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

// ---------- not drawing what nobody can see ----------

/// Every monitor's work area in screen coordinates, in the same order as `monitors()`.
fn screen_work_areas() -> Vec<RECT> {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(m, &mut info).as_bool() {
            (*(out.0 as *mut Vec<RECT>)).push(info.rcWork);
        }
        BOOL(1)
    }
    let mut v: Vec<RECT> = vec![];
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

/// Is the text panel hidden behind ordinary windows (maximised, or as big as the screen) on
/// every screen that shows it? Then the clock and system strip don't need redrawing.
/// Anything see-through, layered, cloaked or minimised never counts, so a wrong guess can only
/// mean "keep drawing".
fn panel_hidden() -> bool {
    struct Look {
        targets: Vec<RECT>,
        covered: Vec<bool>,
    }
    unsafe extern "system" fn each(h: HWND, lp: LPARAM) -> BOOL {
        let look = &mut *(lp.0 as *mut Look);
        if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() || h == ctl() || h == wall() {
            return BOOL(1);
        }
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) != 0 {
            return BOOL(1);
        }
        let mut cloaked = 0u32;
        let _ = windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(h, windows::Win32::Graphics::Dwm::DWMWA_CLOAKED, &mut cloaked as *mut _ as *mut _, 4);
        if cloaked != 0 {
            return BOOL(1);
        }
        let mut class = [0u16; 32];
        let n = GetClassNameW(h, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return BOOL(1);
        }
        let mut r = RECT::default();
        if windows::Win32::Graphics::Dwm::DwmGetWindowAttribute(h, windows::Win32::Graphics::Dwm::DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut _ as *mut _, std::mem::size_of::<RECT>() as u32).is_err() {
            let _ = GetWindowRect(h, &mut r);
        }
        for (t, c) in look.targets.iter().zip(look.covered.iter_mut()) {
            if r.left <= t.left + 2 && r.top <= t.top + 2 && r.right >= t.right - 2 && r.bottom >= t.bottom - 2 {
                *c = true;
            }
        }
        BOOL(if look.covered.iter().all(|c| *c) { 0 } else { 1 })
    }
    let panels = with_app(|a| a.panel_monitors()).unwrap_or_default();
    let works = screen_work_areas();
    let targets: Vec<RECT> = panels.iter().filter_map(|&i| works.get(i).copied()).collect();
    if targets.is_empty() {
        return false;
    }
    let mut look = Look { covered: vec![false; targets.len()], targets };
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut look as *mut _ as isize));
    }
    look.covered.iter().all(|c| *c)
}

/// Remembers a skipped repaint (1 = the panel, 2 = everything) and watches for the desktop
/// coming back.
fn skip_repaint(level: u8) {
    with_app(|a| a.stale = a.stale.max(level));
    unsafe {
        SetTimer(Some(ctl()), TIMER_PEEK, 2000, None);
    }
}

/// If repaints were skipped and the panel can be seen again, draws what was missed.
fn catch_up() {
    let stale = with_app(|a| a.stale).unwrap_or(0);
    if stale == 0 || panel_hidden() {
        return;
    }
    with_app(|a| a.stale = 0);
    unsafe {
        let _ = KillTimer(Some(ctl()), TIMER_PEEK);
    }
    if stale >= 2 {
        invalidate(None);
    } else {
        with_app(|a| a.panel_rects()).unwrap_or_default().into_iter().for_each(|r| invalidate(Some(r)));
    }
}

/// Windows coming to the front, being minimised or moved: the desktop may be visible again.
unsafe extern "system" fn window_event(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    if with_app(|a| a.stale).unwrap_or(0) > 0 {
        catch_up();
    }
}

fn watch_windows() {
    const FOREGROUND: u32 = 0x0003;
    const MOVESIZEEND: u32 = 0x000B;
    const MINIMIZESTART: u32 = 0x0016;
    const MINIMIZEEND: u32 = 0x0017;
    const OUTOFCONTEXT_SKIPOWN: u32 = 0x0002; // WINEVENT_OUTOFCONTEXT (0) | WINEVENT_SKIPOWNPROCESS
    unsafe {
        for (a, b) in [(FOREGROUND, FOREGROUND), (MOVESIZEEND, MOVESIZEEND), (MINIMIZESTART, MINIMIZEEND)] {
            let _ = SetWinEventHook(a, b, None, Some(window_event), 0, 0, OUTOFCONTEXT_SKIPOWN);
        }
    }
}

/// Is a full-screen game, video or presentation running?
fn fullscreen_busy() -> bool {
    unsafe { SHQueryUserNotificationState().map(|s| s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE).unwrap_or(false) }
}

/// The once-a-minute update: times, reminder, notifications, and the smallest repaint needed.
/// Once a day: days until Ramadan and the two Eids (one tiny request, on a short-lived thread).
fn ensure_events() {
    let job = with_app(|a| {
        let (days, _) = a.now();
        let fresh = a.events.map(|(d, _)| d == days).unwrap_or(false);
        if !a.s.show_event || fresh || a.events_fetching || times::utc_seconds() - a.events_try < 300 {
            return None;
        }
        a.events_fetching = true;
        a.events_try = times::utc_seconds();
        let (y, m, d) = times::civil_from_days(days);
        Some((days, format!("{}/calendar/events?date={y:04}-{m:02}-{d:02}&adjustment={}", times::API, a.s.hijri_adjust)))
    })
    .flatten();
    if let Some((days, url)) = job {
        let target = ctl().0 as isize;
        std::thread::spawn(move || {
            let got = ureq::get(&url).timeout(std::time::Duration::from_secs(15)).call().ok().and_then(|r| r.into_json::<serde_json::Value>().ok()).and_then(|v| {
                let d = v.get("data")?;
                let n = |k: &str| d.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
                Some([n("daysUntilRamadan"), n("daysUntilEidAlFitr"), n("daysUntilEidAlAdha")])
            });
            let p = Box::into_raw(Box::new(got.map(|e| (days, e)))) as isize;
            unsafe {
                if PostMessageW(Some(HWND(target as _)), WM_EVENTS, WPARAM(0), LPARAM(p)).is_err() {
                    drop(Box::from_raw(p as *mut Option<(i64, [i64; 3])>));
                }
            }
        });
    }
}

/// Once a day, if switched on: is there a newer release on GitHub? Only for the installed copy.
fn check_update() {
    let go = with_app(|a| {
        let due = a.s.auto_update && a.update.is_none() && times::utc_seconds() - a.update_checked > 86_400;
        if due {
            a.update_checked = times::utc_seconds();
        }
        due
    })
    .unwrap_or(false);
    if !go || !install::is_installed_copy() {
        return;
    }
    // The check runs in a short-lived copy of the app (`--check-update`), so the TLS and JSON
    // work never stays in this process's memory. It leaves its answer in update.json.
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg("--check-update").spawn();
    }
}

/// `--check-update`: ask GitHub for the latest release; if it is newer, save it and tell the
/// running copy. Then exit.
fn check_update_child() {
    let url = "https://api.github.com/repos/Saimon8420/miqaat-wallpaper/releases/latest";
    let found = ureq::get(url)
        .set("User-Agent", "miqaat-wallpaper")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok())
        .and_then(|v| {
            let tag = v.get("tag_name")?.as_str()?.trim_start_matches('v').to_string();
            let asset = v.get("assets")?.as_array()?.iter().find_map(|a| {
                let u = a.get("browser_download_url")?.as_str()?;
                u.ends_with(".exe").then(|| u.to_string())
            })?;
            extras::newer(&tag, install::VERSION).then_some((tag, asset))
        });
    if let Some((v, u)) = found {
        let _ = std::fs::write(settings::data_dir().join("update.json"), serde_json::json!({ "version": v, "url": u }).to_string());
        unsafe {
            if let Ok(h) = FindWindowW(w!("MiqaatWallpaperCtl"), None) {
                let _ = PostMessageW(Some(h), WM_UPDATE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Removes what an update leaves behind: the downloaded installer in %TEMP% and the saved
/// "new version" note once this copy is that version or newer. The installer may still be
/// closing when this runs, so deleting it is retried for a little while on a short thread.
fn clean_after_update() {
    let note = settings::data_dir().join("update.json");
    let stale = std::fs::read_to_string(&note)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|j| j.get("version")?.as_str().map(|v| !extras::newer(v, install::VERSION)))
        .unwrap_or(true);
    if stale {
        let _ = std::fs::remove_file(&note);
    }
    let setup = std::env::temp_dir().join("MiqaatWallpaper-Setup.exe");
    if setup.exists() {
        std::thread::spawn(move || {
            for _ in 0..30 {
                if std::fs::remove_file(&setup).is_ok() || !setup.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
    }
}

/// Downloads the new version and runs it; it closes this copy, replaces it and starts again.
fn run_update(url: String) {
    std::thread::spawn(move || {
        let path = std::env::temp_dir().join("MiqaatWallpaper-Setup.exe");
        let ok = ureq::get(&url)
            .set("User-Agent", "miqaat-wallpaper")
            .timeout(std::time::Duration::from_secs(300))
            .call()
            .ok()
            .and_then(|r| {
                let mut f = std::fs::File::create(&path).ok()?;
                std::io::copy(&mut r.into_reader(), &mut f).ok()
            })
            .map(|n| n > 1_000_000)
            .unwrap_or(false);
        if ok {
            let _ = std::process::Command::new(&path).arg("--silent").spawn();
        } else {
            log("update download failed");
        }
    });
}

fn tick() {
    ensure_times();
    ensure_events();
    check_update();
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
        (if full { None } else { Some(a.panel_rects()) }, tip)
    });
    if let Some((rects, tip)) = plan {
        tray::tip(ctl(), &tip);
        if panel_hidden() {
            skip_repaint(if rects.is_some() { 1 } else { 2 });
            return;
        }
        match rects {
            Some(rs) => rs.into_iter().for_each(|r| invalidate(Some(r))),
            None => invalidate(None),
        }
    }
}

/// Every monitor's rectangle and work area (without the taskbar) in `parent`'s client
/// coordinates, and which one is primary.
fn monitors(parent: HWND) -> (Vec<RECT>, Vec<RECT>, usize) {
    unsafe extern "system" fn each(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(m, &mut info).as_bool() {
            (*(out.0 as *mut Vec<(RECT, RECT, bool)>)).push((info.rcMonitor, info.rcWork, info.dwFlags & 1 == 1));
        }
        BOOL(1)
    }
    let mut found: Vec<(RECT, RECT, bool)> = vec![];
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut found as *mut _ as isize));
    }
    let to_client = |r: RECT| {
        let mut pts = [POINT { x: r.left, y: r.top }, POINT { x: r.right, y: r.bottom }];
        unsafe { MapWindowPoints(None, Some(parent), &mut pts) };
        RECT { left: pts[0].x, top: pts[0].y, right: pts[1].x, bottom: pts[1].y }
    };
    let all: Vec<RECT> = found.iter().map(|(r, _, _)| to_client(*r)).collect();
    let work: Vec<RECT> = found.iter().map(|(_, r, _)| to_client(*r)).collect();
    let primary = found.iter().position(|(_, _, p)| *p).unwrap_or(0);
    (all, work, primary)
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
        let (mons, work, primary) = monitors(host.parent);
        with_app(|a| {
            a.monitors = mons;
            a.work = work;
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
            let moved = with_app(|a| {
                a.ensure_renderer();
                let f = a.frame();
                let st = a.style();
                match &a.renderer {
                    Some(r) => {
                        let before = (r.clock_rect.borrow().clone(), r.strip_rect.borrow().clone());
                        if let Err(e) = r.paint(ps.hdc, ps.rcPaint, &a.monitors, &a.work, &a.panel_monitors(), &f, &st) {
                            log(&format!("paint: {e}"));
                        }
                        let after = (r.clock_rect.borrow().clone(), r.strip_rect.borrow().clone());
                        let whole = a.panel_rects().iter().all(|p| ps.rcPaint.left <= p.left && ps.rcPaint.top <= p.top && ps.rcPaint.right >= p.right && ps.rcPaint.bottom >= p.bottom);
                        (before != after && !before.0.is_empty() && !whole).then(|| a.panel_rects())
                    }
                    None => {
                        log("no renderer");
                        None
                    }
                }
            })
            .flatten();
            let _ = EndPaint(hwnd, &ps);
            let r = ps.rcPaint;
            if (r.right - r.left) as i64 * (r.bottom - r.top) as i64 > 600_000 {
                trim_memory();
            }
            if let Some(rs) = moved {
                for r in rs {
                    let _ = InvalidateRect(Some(hwnd), Some(&r), false);
                }
            }
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
                    if with_app(|a| !a.busy && !a.paused).unwrap_or(false) && panel_hidden() {
                        skip_repaint(1);
                    } else if with_app(|a| !a.busy && !a.paused).unwrap_or(false) {
                        let rs = with_app(|a| a.renderer.as_ref().map(|r| r.clock_rect.borrow().values().copied().collect::<Vec<_>>())).flatten().unwrap_or_default();
                        rs.into_iter().for_each(|r| invalidate(Some(r)));
                    }
                }
                TIMER_MONITOR => {
                    if panel_hidden() {
                        skip_repaint(1);
                        return LRESULT(0);
                    }
                    let r = with_app(|a| {
                        if a.busy || a.paused || (a.s.battery_saver && app::on_battery()) {
                            return None;
                        }
                        a.sysmon.as_mut()?.sample();
                        a.renderer.as_ref().map(|r| r.strip_rect.borrow().values().copied().collect::<Vec<_>>())
                    })
                    .flatten();
                    if let Some(rs) = r {
                        if rs.is_empty() {
                            invalidate(None);
                        } else {
                            rs.into_iter().for_each(|r| invalidate(Some(r)));
                        }
                    }
                }
                TIMER_PEEK => catch_up(),
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
            let f = *Box::from_raw(lp.0 as *mut Fetched);
            let mut ok = true;
            with_app(|a| {
                a.fetching = false;
                match f.current {
                    Some(Ok(m)) => {
                        a.month = Some(m);
                        a.failed = false;
                        a.rotate_reminder(true);
                    }
                    Some(Err(_)) => {
                        a.failed = a.month.is_none();
                        ok = false;
                    }
                    None => {}
                }
                for m in f.near {
                    if !a.near.iter().any(|x| x.key == m.key) {
                        a.near.push(m);
                    }
                }
                if a.near.len() >= 2 {
                    a.near_key = f.near_key;
                } else {
                    ok = false;
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
                        let custom = a.s.scene == "custom";
                        match a.scene_key.get(a.primary).copied().unwrap_or(app::Pic::Photo(0)) {
                            app::Pic::Cal(i) => (a.paused, format!("Calligraphy · {}", assets::CALLIGRAPHY[i].name), String::new()),
                            app::Pic::Photo(_) if custom => (a.paused, "your own photo".to_string(), String::new()),
                            app::Pic::Photo(i) => {
                                let s = &assets::SCENES[i];
                                (a.paused, format!("{} · {}", s.name, s.credit), s.source.to_string())
                            }
                        }
                    })
                    .unwrap_or((false, String::new(), String::new()));
                    let update = with_app(|a| a.update.as_ref().map(|u| u.0.clone())).flatten();
                    match tray::menu(hwnd, paused, &credit, update.as_deref()) {
                        tray::CMD_UPDATE => {
                            if let Some(url) = with_app(|a| a.update.as_ref().map(|u| u.1.clone())).flatten() {
                                tray::notify(hwnd, "Updating Miqaat Wallpaper", "Downloading the new version; it restarts by itself.");
                                run_update(url);
                            }
                        }
                        tray::CMD_SETTINGS => settings_ui::open(),
                        tray::CMD_NEXT_SCENE => {
                            with_app(|a| {
                                if let (Some(id), "keep") = (a.s.scene.strip_prefix("cal:"), a.s.rotate.as_str()) {
                                    let list = calligraphy::everyday();
                                    let k = calligraphy::index(id).and_then(|i| list.iter().position(|&x| x == i)).map(|k| (k + 1) % list.len()).unwrap_or(0);
                                    a.s.scene = format!("cal:{}", assets::CALLIGRAPHY[list[k]].id);
                                    a.s.save();
                                } else if a.s.scene == "custom" || a.s.rotate == "keep" {
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
                                a.near.clear();
                                a.near_key.clear();
                                a.near_retry = 0;
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
        WM_EVENTS => {
            let got = *Box::from_raw(lp.0 as *mut Option<(i64, [i64; 3])>);
            with_app(|a| {
                a.events_fetching = false;
                if let Some(e) = got {
                    a.events = Some(e);
                    let _ = std::fs::write(settings::data_dir().join("events.json"), serde_json::to_string(&e).unwrap_or_default());
                }
            });
            if got.is_some() {
                let r = with_app(|a| a.panel_rects()).unwrap_or_default();
                r.into_iter().for_each(|r| invalidate(Some(r)));
            }
            LRESULT(0)
        }
        WM_UPDATE => {
            let saved = std::fs::read_to_string(settings::data_dir().join("update.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            let Some((v, url)) = saved.and_then(|j| Some((j.get("version")?.as_str()?.to_string(), j.get("url")?.as_str()?.to_string()))) else { return LRESULT(0) };
            if !extras::newer(&v, install::VERSION) {
                return LRESULT(0);
            }
            with_app(|a| a.update = Some((v.clone(), url)));
            tray::notify(hwnd, &format!("Miqaat Wallpaper {v} is available"), "Right-click the moon icon near the clock and choose Update.");
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
    a.work = a.monitors.clone();
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
    // --at HH:MM or HH:MM:SS
    a.fake_now = arg_after(args, "--at").and_then(|v| {
        let p: Vec<u32> = v.split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
        Some(p.first()? * 3600 + p.get(1)? * 60 + p.get(2).copied().unwrap_or(0))
    });
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
    if has("--check-update") {
        check_update_child();
        return Ok(());
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
        watch_windows();
        SetTimer(Some(ctl), TIMER_MINUTE, ms_to_next_minute(), None);
        if has("--welcome") {
            tray::notify(ctl, "Miqaat Wallpaper is on your desktop", "Right-click the moon icon in the taskbar corner for settings.");
            settings_ui::open();
        } else if has("--updated") {
            tray::notify(ctl, &format!("Miqaat Wallpaper {}", install::VERSION), "Updated. Your settings were kept.");
        } else if let Some(i) = args.iter().position(|a| a == "--settings") {
            settings_ui::open_at(args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(0));
        }
        if install::is_installed_copy() {
            clean_after_update();
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
