//! CPU, memory, GPU, disk and network for the optional system strip.
//!
//! CPU and memory come straight from Windows; GPU, disk and network from performance
//! counters (PDH). Nothing is sampled unless the strip is switched on.

use std::collections::VecDeque;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Performance::*;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

pub const HISTORY: usize = 40;

#[derive(Default, Clone)]
pub struct Sample {
    pub cpu: f32,
    pub mem: f32,
    pub mem_used_gb: f32,
    pub mem_total_gb: f32,
    pub gpu: f32,
    pub disk: f32,
    pub down: f64,
    pub up: f64,
}

pub struct Monitor {
    query: isize,
    gpu: isize,
    disk: isize,
    down: isize,
    up: isize,
    last_cpu: Option<(u64, u64)>,
    pub now: Sample,
    /// 0..1 values for the little history lines: cpu, mem, gpu, disk, network.
    pub history: [VecDeque<f32>; 5],
}

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

impl Monitor {
    pub fn new() -> Self {
        let mut m = Monitor { query: 0, gpu: 0, disk: 0, down: 0, up: 0, last_cpu: None, now: Sample::default(), history: Default::default() };
        unsafe {
            let mut q = PDH_HQUERY::default();
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut q) == 0 {
                m.query = q.0 as isize;
                let add = |path: PCWSTR| {
                    let mut c = PDH_HCOUNTER::default();
                    if PdhAddEnglishCounterW(q, path, 0, &mut c) == 0 { c.0 as isize } else { 0 }
                };
                m.gpu = add(w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"));
                m.disk = add(w!("\\PhysicalDisk(_Total)\\% Disk Time"));
                m.down = add(w!("\\Network Interface(*)\\Bytes Received/sec"));
                m.up = add(w!("\\Network Interface(*)\\Bytes Sent/sec"));
                let _ = PdhCollectQueryData(q);
            }
        }
        m
    }

    /// Sum of a wildcard counter over all its instances.
    unsafe fn sum(counter: isize) -> f64 {
        if counter == 0 {
            return 0.0;
        }
        let c = PDH_HCOUNTER(counter as *mut _);
        let (mut size, mut count) = (0u32, 0u32);
        let _ = PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, None);
        if size == 0 {
            return 0.0;
        }
        let mut buf = vec![0u8; size as usize];
        let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
        if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
            return 0.0;
        }
        (0..count as usize).map(|i| (*items.add(i)).FmtValue.Anonymous.doubleValue).filter(|v| v.is_finite()).sum()
    }

    pub fn sample(&mut self) {
        let mut s = Sample::default();
        unsafe {
            let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                let (i, t) = (ft(idle), ft(kernel) + ft(user));
                if let Some((pi, pt)) = self.last_cpu {
                    let (di, dt) = (i.saturating_sub(pi), t.saturating_sub(pt));
                    if dt > 0 {
                        s.cpu = (1.0 - di as f32 / dt as f32).clamp(0.0, 1.0);
                    }
                }
                self.last_cpu = Some((i, t));
            }
            let mut ms = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            if GlobalMemoryStatusEx(&mut ms).is_ok() {
                s.mem = ms.dwMemoryLoad as f32 / 100.0;
                s.mem_total_gb = ms.ullTotalPhys as f32 / 1073741824.0;
                s.mem_used_gb = (ms.ullTotalPhys - ms.ullAvailPhys) as f32 / 1073741824.0;
            }
            if self.query != 0 && PdhCollectQueryData(PDH_HQUERY(self.query as *mut _)) == 0 {
                s.gpu = (Self::sum(self.gpu) as f32 / 100.0).clamp(0.0, 1.0);
                s.disk = (Self::sum(self.disk) as f32 / 100.0).clamp(0.0, 1.0);
                s.down = Self::sum(self.down);
                s.up = Self::sum(self.up);
            }
        }
        // Network history on a log scale: 1 KB/s is near the bottom, 100 MB/s at the top.
        let net = (((s.down + s.up).max(1.0).log10() - 3.0) / 5.0).clamp(0.0, 1.0) as f32;
        for (h, v) in self.history.iter_mut().zip([s.cpu, s.mem, s.gpu, s.disk, net]) {
            if h.len() == HISTORY {
                h.pop_front();
            }
            h.push_back(v);
        }
        self.now = s;
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        if self.query != 0 {
            unsafe {
                let _ = PdhCloseQuery(PDH_HQUERY(self.query as *mut _));
            }
        }
    }
}

/// "1.2 MB/s", "320 KB/s".
pub fn rate(bytes: f64) -> String {
    if bytes >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes / 1_048_576.0)
    } else {
        format!("{:.0} KB/s", bytes / 1024.0)
    }
}
