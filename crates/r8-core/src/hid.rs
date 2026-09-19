//! HID protocol for Retro R8 Mouse Adapter (PID 0x5206), Usage Page 0xFF00.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

pub const VENDOR: u16 = 0x2dc8;
pub const PRODUCT: u16 = 0x5206;

/// Firmware button indices: 0 left, 1 right, 2 middle, 3 side, 4 extra, 5 dpi(?)
pub const BTN_INDEX_SIDE: u8 = 3;
pub const BTN_INDEX_EXTRA: u8 = 4;

pub const DEFAULT_DPI_STAGES: [u16; 6] = [800, 1200, 1600, 2400, 3200, 6400];

/// LED stage color hex values (names via `stage_color_name` in i18n).
pub const STAGE_COLOR_HEX: [&str; 6] = [
    "#e6c200", "#3cb043", "#e67e22", "#e91e8c", "#3498db", "#9b59b6",
];

/// English name + hex (prefer `STAGE_COLOR_HEX` + i18n for UI).
pub const STAGE_COLORS: [(&str, &str); 6] = [
    ("Yellow", "#e6c200"),
    ("Green", "#3cb043"),
    ("Orange", "#e67e22"),
    ("Pink", "#e91e8c"),
    ("Blue", "#3498db"),
    ("Purple", "#9b59b6"),
];

#[derive(Debug, Error)]
pub enum HidError {
    #[error("no R8 config hidraw found")]
    NotFound,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad HID reply")]
    BadReply,
}

#[derive(Debug, Clone)]
pub struct DpiState {
    pub index: usize,
    pub current_dpi: u16,
    pub stages: Vec<u16>,
}

/// Battery state from the mouse (vendor HID, same channel as DPI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryState {
    /// 0–100
    pub percent: u8,
    pub charging: bool,
    /// False if the mouse replies but has no real value yet (empty payload).
    pub known: bool,
}

pub fn find_config_hidraw() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/sys/class/hidraw").ok()?;
    for ent in entries.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("hidraw") {
            continue;
        }
        let uevent = ent.path().join("device/uevent");
        let text = std::fs::read_to_string(&uevent).ok()?;
        if !text.contains("5206") || !text.to_uppercase().contains("2DC8") {
            continue;
        }
        let desc_path = ent.path().join("device/report_descriptor");
        let desc = std::fs::read(&desc_path).ok()?;
        // Usage Page 0xFF00
        if desc.len() >= 3 && desc[0] == 0x06 && desc[1] == 0x00 && desc[2] == 0xff {
            return Some(PathBuf::from(format!("/dev/{name}")));
        }
    }
    None
}

fn open_rw(path: &Path) -> Result<std::fs::File, HidError> {
    Ok(OpenOptions::new().read(true).write(true).open(path)?)
}

fn xfer(file: &mut std::fs::File, pkt: &[u8]) -> Result<Vec<Vec<u8>>, HidError> {
    // drain
    let fd = file.as_raw_fd();
    let mut drain_buf = [0u8; 64];
    for _ in 0..8 {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut pfd, 1, 0) };
        if n <= 0 {
            break;
        }
        let _ = file.read(&mut drain_buf);
    }

    let mut buf = [0u8; 64];
    let n = pkt.len().min(64);
    buf[..n].copy_from_slice(&pkt[..n]);
    file.write_all(&buf)?;

    std::thread::sleep(Duration::from_millis(50));

    let mut out = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(350);
    while std::time::Instant::now() < deadline {
        let wait = deadline.saturating_duration_since(std::time::Instant::now());
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let n = unsafe { libc::poll(&mut pfd, 1, wait.as_millis() as i32) };
        if n <= 0 {
            break;
        }
        let mut rbuf = [0u8; 64];
        match file.read(&mut rbuf) {
            Ok(0) => break,
            Ok(_) => out.push(rbuf.to_vec()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(out)
}

pub fn read_dpi() -> Result<DpiState, HidError> {
    let path = find_config_hidraw().ok_or(HidError::NotFound)?;
    let mut f = open_rw(&path)?;
    // ReadDpiIndex: 01 06 12 01
    let replies = xfer(&mut f, &[0x01, 0x06, 0x12, 0x01])?;
    for r in replies {
        if r.len() >= 16 && r[0] == 0x01 && r[1] == 0x06 && r[2] == 0x12 {
            let count = r[4] as usize;
            let index = r[5] as usize;
            let current = u16::from_le_bytes([r[6], r[7]]);
            let mut stages = Vec::new();
            for i in 0..count {
                let off = 8 + i * 2;
                if off + 1 < r.len() {
                    stages.push(u16::from_le_bytes([r[off], r[off + 1]]));
                }
            }
            if stages.is_empty() {
                stages = DEFAULT_DPI_STAGES.to_vec();
            }
            return Ok(DpiState {
                index: index.min(stages.len().saturating_sub(1)),
                current_dpi: current,
                stages,
            });
        }
    }
    Err(HidError::BadReply)
}

pub fn read_button_map(btn_index: u8) -> Result<u16, HidError> {
    let path = find_config_hidraw().ok_or(HidError::NotFound)?;
    let mut f = open_rw(&path)?;
    let replies = xfer(&mut f, &[0x01, 0x06, 0x10, 0x01, 0x00, btn_index])?;
    for r in replies {
        if r.len() >= 8 && r[0] == 0x01 && r[2] == 0x10 && r[5] == btn_index {
            return Ok(u16::from_le_bytes([r[6], r[7]]));
        }
    }
    Err(HidError::BadReply)
}

/// Write firmware map for one button. Format: `01 06 10 04 00 | btn val_lo val_hi 00 00 00`
pub fn write_button_map(btn_index: u8, value: u16) -> Result<(), HidError> {
    let path = find_config_hidraw().ok_or(HidError::NotFound)?;
    let mut f = open_rw(&path)?;
    let pkt = [
        0x01,
        0x06,
        0x10,
        0x04,
        0x00,
        btn_index,
        (value & 0xff) as u8,
        (value >> 8) as u8,
        0,
        0,
        0,
    ];
    let _ = xfer(&mut f, &pkt)?;
    // verify
    std::thread::sleep(Duration::from_millis(40));
    let got = read_button_map(btn_index)?;
    if got != value {
        // sometimes an extra attempt is needed
        let _ = xfer(&mut open_rw(&path)?, &pkt)?;
        std::thread::sleep(Duration::from_millis(40));
        let got2 = read_button_map(btn_index)?;
        if got2 != value {
            return Err(HidError::BadReply);
        }
    }
    Ok(())
}

/// Save side buttons in mouse firmware (persists without software).
pub fn write_side_button_maps(side: u16, extra: u16) -> Result<(), HidError> {
    write_button_map(BTN_INDEX_SIDE, side)?;
    write_button_map(BTN_INDEX_EXTRA, extra)?;
    Ok(())
}

/// Read battery. Ultimate Software: `_ReadBAT` → opcode `0x1A`.
/// R8-framing (som DPI): `01 06 1A 01` → svar `01 06 1A 11 | percent | status | …`.
pub fn read_battery() -> Result<BatteryState, HidError> {
    let path = find_config_hidraw().ok_or(HidError::NotFound)?;
    let mut f = open_rw(&path)?;
    let replies = xfer(&mut f, &[0x01, 0x06, 0x1a, 0x01])?;
    for r in replies {
        if r.len() < 6 || r[0] != 0x01 || r[1] != 0x06 || r[2] != 0x1a {
            continue;
        }
        // Accept both echo-sub (0x01) and data-sub (0x11)
        if r[3] != 0x01 && r[3] != 0x11 {
            continue;
        }
        let percent = r[4].min(100);
        let status = r.get(5).copied().unwrap_or(0);
        let charging = status & 0x01 != 0;
        let known = r[4..6.min(r.len())].iter().any(|&b| b != 0) || percent > 0;
        return Ok(BatteryState {
            percent,
            charging,
            known,
        });
    }
    Err(HidError::BadReply)
}
