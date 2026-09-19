//! Software remapper-daemon for 8BitDo Retro R8.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use evdev::{
    uinput::VirtualDeviceBuilder, AttributeSet, Device, EventType, InputEvent, Key,
    RelativeAxisType,
};
use r8_core::{load_config, Action, Config};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "r8d", about = "8BitDo R8 remapper daemon")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
}

fn config_path(args: &Args) -> PathBuf {
    args.config
        .clone()
        .unwrap_or_else(|| r8_core::default_config_dir().join(r8_core::CONFIG_NAME))
}

fn find_mouse() -> Result<Device> {
    let by_id = PathBuf::from("/dev/input/by-id");
    if by_id.is_dir() {
        for ent in std::fs::read_dir(&by_id)?.flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name.contains("Retro_R8_Mouse_Adapter") && name.ends_with("-event-mouse") {
                return Device::open(ent.path()).context("open mouse");
            }
        }
    }
    bail!("Ingen Retro R8-mus fundet");
}

fn build_uinput() -> Result<evdev::uinput::VirtualDevice> {
    let mut keys = AttributeSet::<Key>::new();
    for k in [
        Key::BTN_LEFT,
        Key::BTN_RIGHT,
        Key::BTN_MIDDLE,
        Key::BTN_SIDE,
        Key::BTN_EXTRA,
        Key::KEY_LEFTCTRL,
        Key::KEY_C,
        Key::KEY_V,
        Key::KEY_X,
        Key::KEY_Z,
        Key::KEY_T,
        Key::KEY_W,
        Key::KEY_ENTER,
        Key::KEY_ESC,
        Key::KEY_VOLUMEUP,
        Key::KEY_VOLUMEDOWN,
        Key::KEY_MUTE,
        Key::KEY_PLAYPAUSE,
        Key::KEY_NEXTSONG,
        Key::KEY_PREVIOUSSONG,
    ] {
        keys.insert(k);
    }
    let mut rel = AttributeSet::<RelativeAxisType>::new();
    for a in [
        RelativeAxisType::REL_X,
        RelativeAxisType::REL_Y,
        RelativeAxisType::REL_WHEEL,
        RelativeAxisType::REL_HWHEEL,
        RelativeAxisType::REL_WHEEL_HI_RES,
        RelativeAxisType::REL_HWHEEL_HI_RES,
    ] {
        rel.insert(a);
    }

    let device = VirtualDeviceBuilder::new()?
        .name("8BitDo R8 (software)")
        .with_keys(&keys)?
        .with_relative_axes(&rel)?
        .build()?;
    Ok(device)
}
fn emit_key(ui: &mut evdev::uinput::VirtualDevice, key: Key, value: i32) -> Result<()> {
    ui.emit(&[InputEvent::new(EventType::KEY, key.code(), value)])?;
    Ok(())
}

fn syn(ui: &mut evdev::uinput::VirtualDevice) -> Result<()> {
    ui.emit(&[InputEvent::new(EventType::SYNCHRONIZATION, 0, 0)])?;
    Ok(())
}

fn tap(ui: &mut evdev::uinput::VirtualDevice, key: Key) -> Result<()> {
    emit_key(ui, key, 1)?;
    emit_key(ui, key, 0)?;
    syn(ui)
}

fn tap_combo(ui: &mut evdev::uinput::VirtualDevice, keys: &[Key]) -> Result<()> {
    for k in keys {
        emit_key(ui, *k, 1)?;
    }
    for k in keys.iter().rev() {
        emit_key(ui, *k, 0)?;
    }
    syn(ui)
}

fn emit_action(
    ui: &mut evdev::uinput::VirtualDevice,
    action: Action,
    value: i32,
    original: Key,
) -> Result<()> {
    match action {
        Action::Default => emit_key(ui, original, value)?,
        Action::Disabled => {}
        Action::Left => emit_key(ui, Key::BTN_LEFT, value)?,
        Action::Right => emit_key(ui, Key::BTN_RIGHT, value)?,
        Action::Middle => emit_key(ui, Key::BTN_MIDDLE, value)?,
        Action::Back => emit_key(ui, Key::BTN_SIDE, value)?,
        Action::Forward => emit_key(ui, Key::BTN_EXTRA, value)?,
        other if value == 1 => match other {
            Action::Copy => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_C])?,
            Action::Paste => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_V])?,
            Action::Cut => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_X])?,
            Action::Undo => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_Z])?,
            Action::NewTab => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_T])?,
            Action::CloseTab => tap_combo(ui, &[Key::KEY_LEFTCTRL, Key::KEY_W])?,
            Action::VolUp => tap(ui, Key::KEY_VOLUMEUP)?,
            Action::VolDown => tap(ui, Key::KEY_VOLUMEDOWN)?,
            Action::Mute => tap(ui, Key::KEY_MUTE)?,
            Action::PlayPause => tap(ui, Key::KEY_PLAYPAUSE)?,
            Action::NextTrack => tap(ui, Key::KEY_NEXTSONG)?,
            Action::PrevTrack => tap(ui, Key::KEY_PREVIOUSSONG)?,
            Action::Enter => tap(ui, Key::KEY_ENTER)?,
            Action::Escape => tap(ui, Key::KEY_ESC)?,
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

fn run_loop(cfg: &Config, stop: Arc<AtomicBool>) -> Result<()> {
    let mut src = find_mouse()?;
    src.grab().context("grab mouse")?;
    let mut ui = build_uinput()?;
    info!(
        "remapper aktiv side={:?} extra={:?}",
        cfg.buttons.side, cfg.buttons.extra
    );

    while !stop.load(Ordering::Relaxed) {
        match src.fetch_events() {
            Ok(iter) => {
                for ev in iter {
                    if ev.event_type() == EventType::KEY {
                        let code = Key::new(ev.code());
                        if code == Key::BTN_SIDE {
                            emit_action(&mut ui, cfg.buttons.side, ev.value(), Key::BTN_SIDE)?;
                        } else if code == Key::BTN_EXTRA {
                            emit_action(&mut ui, cfg.buttons.extra, ev.value(), Key::BTN_EXTRA)?;
                        } else {
                            ui.emit(&[ev])?;
                        }
                    } else if ev.event_type() == EventType::SYNCHRONIZATION {
                        syn(&mut ui)?;
                    } else {
                        ui.emit(&[ev])?;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                warn!("fetch_events: {e}");
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    let _ = src.ungrab();
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let args = Args::parse();
    let path = config_path(&args);
    let cfg = load_config(&path);
    if !cfg.software_remap {
        info!("software_remap=false — daemon afslutter");
        return Ok(());
    }

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed))?;
    }

    info!("config {}", path.display());
    run_loop(&cfg, stop)?;
    info!("stoppet");
    Ok(())
}
