//! Settings GUI for 8BitDo Retro R8 — N Edition look.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Frame, Margin, RichText, Rounding, Stroke, Vec2};
use r8_core::{
    hardware_mouse_value, load_config, read_battery, read_button_map, read_dpi, save_config,
    stage_color_name, ui_text, write_side_button_maps, Action, BatteryState, Config, DpiState,
    Lang, UiText, ACTION_LIST, STAGE_COLOR_HEX,
};

// ── N Edition palette (from official product shots / 8bitdo-linux) ──────────
const BG: Color32 = Color32::from_rgb(0x2a, 0x29, 0x30); // charcoal case
const BG_DEEP: Color32 = Color32::from_rgb(0x1e, 0x1d, 0x24);
const CARD: Color32 = Color32::from_rgb(0x3a, 0x39, 0x42);
const CARD_EDGE: Color32 = Color32::from_rgb(0x53, 0x52, 0x5e);
const CREAM: Color32 = Color32::from_rgb(0xec, 0xe7, 0xda); // keycap cream
const CREAM_DIM: Color32 = Color32::from_rgb(0xc8, 0xc1, 0xb0);
const NES_RED: Color32 = Color32::from_rgb(0xdd, 0x41, 0x36);
const NES_RED_DEEP: Color32 = Color32::from_rgb(0x9a, 0x2a, 0x22);
const MUTED: Color32 = Color32::from_rgb(0x9a, 0x96, 0xa0);
const OK_GREEN: Color32 = Color32::from_rgb(0x46, 0xd6, 0x7a);

fn config_path() -> PathBuf {
    r8_core::default_config_dir().join(r8_core::CONFIG_NAME)
}

fn pid_path() -> PathBuf {
    r8_core::default_config_dir().join("r8d.pid")
}

fn daemon_running() -> bool {
    let Ok(s) = std::fs::read_to_string(pid_path()) else {
        return false;
    };
    let Ok(pid) = s.trim().parse::<i32>() else {
        return false;
    };
    unsafe { libc::kill(pid, 0) == 0 }
}

fn stop_daemon() {
    if let Ok(s) = std::fs::read_to_string(pid_path()) {
        if let Ok(pid) = s.trim().parse::<i32>() {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    }
    let _ = std::fs::remove_file(pid_path());
}

fn which_bin(name: &str) -> Option<PathBuf> {
    let out = Command::new("which").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(PathBuf::from(s))
}

fn start_daemon(cfg_path: &std::path::Path) -> Result<(), String> {
    stop_daemon();
    let bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("r8d")))
        .filter(|p| p.exists())
        .or_else(|| which_bin("r8d"))
        .ok_or_else(|| "r8d not found — run packaging/install.sh".to_string())?;
    let child = Command::new(&bin)
        .arg("--config")
        .arg(cfg_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(r8_core::default_config_dir());
    let _ = std::fs::write(pid_path(), format!("{}\n", child.id()));
    Ok(())
}

fn hex_color(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return Color32::WHITE;
    }
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
    Color32::from_rgb(r, g, b)
}

fn action_msg(template: &str, action: Action, lang: Lang) -> String {
    template.replace("{action}", action.label_for(lang))
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    Frame::default()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, CARD_EDGE))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(14.0, 14.0))
        .show(ui, |ui| {
            // Stay within parent width (avoid clipping on the right edge).
            ui.set_max_width(ui.available_width().max(1.0));
            add(ui);
        });
}

const DPI_CHIP_H: f32 = 80.0;
const DPI_CHIP_MIN_W: f32 = 110.0;
const DPI_COL_GAP: f32 = 10.0;
const DPI_ROW_GAP: f32 = 16.0;
/// Max content width — avoids stretched dropdowns on Full HD.
const CONTENT_MAX_W: f32 = 580.0;

/// Uniform DPI grid: reserves total height (rows + gaps) so nothing overlaps.
fn dpi_stage_grid(ui: &mut egui::Ui, stages: &[u16], active_idx: usize, lang: Lang) {
    let t = ui_text(lang);
    let n = stages.len();
    if n == 0 {
        return;
    }
    let avail = ui.available_width().max(1.0);

    let mut cols = 1usize;
    for c in (2..=n).rev() {
        let need = DPI_CHIP_MIN_W * c as f32 + DPI_COL_GAP * (c as f32 - 1.0);
        if need <= avail + 0.5 {
            cols = c;
            break;
        }
    }
    let cell_w = if cols == 1 {
        avail.min(160.0).max(DPI_CHIP_MIN_W.min(avail))
    } else {
        ((avail - DPI_COL_GAP * (cols as f32 - 1.0)) / cols as f32).floor()
    };

    let nrows = n.div_ceil(cols);
    let total_h =
        nrows as f32 * DPI_CHIP_H + nrows.saturating_sub(1) as f32 * DPI_ROW_GAP;
    let grid_w = if cols == 1 {
        cell_w
    } else {
        (cell_w * cols as f32 + DPI_COL_GAP * (cols as f32 - 1.0)).min(avail)
    };
    // Single allocation for the whole grid — pushes text below down correctly.
    let (area, _) = ui.allocate_exact_size(Vec2::new(grid_w, total_h), egui::Sense::hover());

    for row_i in 0..nrows {
        let y = area.top() + row_i as f32 * (DPI_CHIP_H + DPI_ROW_GAP);
        for c in 0..cols {
            let i = row_i * cols + c;
            if i >= n {
                break;
            }
            let x = area.left() + c as f32 * (cell_w + DPI_COL_GAP);
            let rect =
                egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(cell_w, DPI_CHIP_H));
            let dpi = stages[i];
            let name = stage_color_name(lang, i);
            let hex = STAGE_COLOR_HEX.get(i).copied().unwrap_or("#888");
            let col = hex_color(hex);
            paint_dpi_chip(ui, rect, name, dpi, col, i == active_idx, t.dpi_active);
        }
    }
}

fn paint_dpi_chip(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    name: &str,
    dpi: u16,
    col: Color32,
    active: bool,
    active_label: &str,
) {
    let rounding = Rounding::same(8.0);
    let painter = ui.painter();
    if active {
        painter.rect(rect, rounding, col, Stroke::new(2.5_f32, CREAM));
    } else {
        painter.rect(rect, rounding, BG_DEEP, Stroke::new(1.0_f32, CARD_EDGE));
        let stripe = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 1.5, rect.top() + 1.5),
            egui::pos2(rect.right() - 1.5, rect.top() + 7.0),
        );
        painter.rect_filled(
            stripe,
            Rounding {
                nw: 6.0,
                ne: 6.0,
                sw: 0.0,
                se: 0.0,
            },
            col,
        );
    }

    // Painter text only — no ui.put/allocate (that broke row spacing).
    let text_c = Color32::WHITE;
    let dpi_c = if active { Color32::WHITE } else { CREAM };
    let x = rect.left() + 10.0;
    let mut y = rect.top() + if active { 10.0 } else { 16.0 };

    if active {
        painter.text(
            egui::pos2(x, y),
            egui::Align2::LEFT_TOP,
            active_label,
            egui::FontId::proportional(9.0),
            text_c,
        );
        y += 14.0;
    }
    painter.text(
        egui::pos2(x, y),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::proportional(13.0),
        text_c,
    );
    y += 20.0;
    painter.text(
        egui::pos2(x, y),
        egui::Align2::LEFT_TOP,
        format!("{dpi}"),
        egui::FontId::proportional(15.0),
        dpi_c,
    );
}

/// Button row: side by side when space allows, otherwise full-width stack.
fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, bool /*full_width*/)) {
    let avail = ui.available_width().max(1.0);
    let stack = avail < 520.0;
    if stack {
        ui.vertical(|ui| {
            ui.set_max_width(avail);
            ui.spacing_mut().item_spacing.y = 8.0;
            add(ui, true);
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(avail);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            add(ui, false);
        });
    }
}

fn muted_label(ui: &mut egui::Ui, text: &str, size: f32) {
    ui.add(egui::Label::new(RichText::new(text).color(MUTED).size(size)).wrap());
}

const BTN_ROUND: f32 = 4.0;
const BTN_HEIGHT: f32 = 42.0;

fn primary_button(ui: &mut egui::Ui, text: &str, full_width: bool) -> egui::Response {
    let w = if full_width {
        ui.available_width()
    } else {
        0.0
    };
    ui.add(
        egui::Button::new(RichText::new(text).color(Color32::WHITE).strong().size(14.0))
            .fill(NES_RED)
            .stroke(Stroke::new(1.0_f32, NES_RED_DEEP))
            .rounding(Rounding::same(BTN_ROUND))
            .min_size(Vec2::new(w, BTN_HEIGHT)),
    )
}

fn secondary_button(ui: &mut egui::Ui, text: &str, full_width: bool) -> egui::Response {
    let w = if full_width {
        ui.available_width()
    } else {
        0.0
    };
    ui.add(
        egui::Button::new(RichText::new(text).color(CREAM).size(14.0))
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0_f32, CARD_EDGE))
            .rounding(Rounding::same(BTN_ROUND))
            .min_size(Vec2::new(w, BTN_HEIGHT)),
    )
}

/// Red text link under a save button (not a filled button).
fn reset_link(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let response = ui.add(
        egui::Label::new(RichText::new(text).color(NES_RED).size(13.0).strong())
            .sense(egui::Sense::click()),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfirmKind {
    ResetSoftware,
    ResetHardware,
}

enum PollMsg {
    Dpi(Result<DpiState, String>),
    Battery(Result<BatteryState, String>),
}

struct AppState {
    cfg: Config,
    cfg_path: PathBuf,
    status: String,
    dpi: Option<DpiState>,
    dpi_error: String,
    battery: Option<BatteryState>,
    message: String,
    poll_tx: Sender<()>,
    poll_rx: Receiver<PollMsg>,
    last_status: Instant,
    last_poll: Instant,
    combo_open: bool,
    confirm: Option<ConfirmKind>,
}

impl AppState {
    fn new() -> Self {
        let (req_tx, req_rx) = mpsc::channel::<()>();
        let (poll_tx, poll_rx) = mpsc::channel::<PollMsg>();
        thread::spawn(move || {
            let mut tick: u64 = 0;
            while req_rx.recv().is_ok() {
                tick = tick.wrapping_add(1);
                // DPI every tick; battery only every 15th (~6s) — spares the radio link.
                match read_dpi() {
                    Ok(s) => {
                        let _ = poll_tx.send(PollMsg::Dpi(Ok(s)));
                    }
                    Err(e) => {
                        let _ = poll_tx.send(PollMsg::Dpi(Err(e.to_string())));
                    }
                }
                // Battery: poll rarely — R8 often returns empty payload over 2.4 GHz.
                if tick % 15 == 1 {
                    match read_battery() {
                        Ok(s) => {
                            let _ = poll_tx.send(PollMsg::Battery(Ok(s)));
                        }
                        Err(e) => {
                            let _ = poll_tx.send(PollMsg::Battery(Err(e.to_string())));
                        }
                    }
                }
            }
        });
        let _ = req_tx.send(());

        let cfg_path = config_path();
        let mut app = Self {
            cfg: load_config(&cfg_path),
            cfg_path,
            status: String::new(),
            dpi: None,
            dpi_error: String::new(),
            battery: None,
            message: String::new(),
            poll_tx: req_tx,
            poll_rx,
            last_status: Instant::now() - Duration::from_secs(10),
            last_poll: Instant::now() - Duration::from_secs(10),
            combo_open: false,
            confirm: None,
        };
        app.refresh_status();
        app
    }

    fn lang(&self) -> Lang {
        self.cfg.language
    }

    fn t(&self) -> UiText {
        ui_text(self.lang())
    }

    fn refresh_status(&mut self) {
        let t = self.t();
        let mouse = std::fs::read_dir("/dev/input/by-id")
            .ok()
            .map(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.contains("Retro_R8_Mouse") && n.ends_with("-event-mouse")
                })
            })
            .unwrap_or(false);
        let mouse_s = if mouse {
            t.mouse_connected
        } else {
            t.mouse_missing
        };
        let daemon = if daemon_running() {
            t.daemon_running
        } else {
            t.daemon_stopped
        };
        self.status = format!("N Edition  ·  {mouse_s}  ·  {daemon}");
        self.last_status = Instant::now();
    }

    fn pump_poll(&mut self) {
        while let Ok(msg) = self.poll_rx.try_recv() {
            match msg {
                PollMsg::Dpi(Ok(s)) => {
                    self.dpi = Some(s);
                    self.dpi_error.clear();
                }
                PollMsg::Dpi(Err(e)) => {
                    self.dpi = None;
                    self.dpi_error = e;
                }
                PollMsg::Battery(Ok(s)) => {
                    self.battery = Some(s);
                }
                PollMsg::Battery(Err(_)) => {
                    self.battery = None;
                }
            }
        }
        // At most one HID round about every 2 seconds (not every frame).
        if !self.combo_open && self.last_poll.elapsed() > Duration::from_secs(2) {
            let _ = self.poll_tx.send(());
            self.last_poll = Instant::now();
        }
    }

    fn language_widget(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("language")
            .width(100.0)
            .selected_text(
                RichText::new(self.cfg.language.native_name())
                    .color(CREAM)
                    .size(13.0),
            )
            .show_ui(ui, |ui| {
                ui.visuals_mut().selection.bg_fill = NES_RED.gamma_multiply(0.65);
                ui.visuals_mut().selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
                for l in Lang::ALL {
                    let selected = self.cfg.language == l;
                    let text = RichText::new(l.native_name())
                        .size(13.0)
                        .color(if selected {
                            Color32::WHITE
                        } else {
                            CREAM
                        });
                    if ui.selectable_label(selected, text).clicked() && !selected {
                        self.cfg.language = l;
                        let _ = save_config(&self.cfg_path, &self.cfg);
                    }
                }
            });
    }

    fn battery_widget(&self, ui: &mut egui::Ui) {
        // R8 does not expose battery over 2.4 GHz (ReadBAT = empty payload).
        // Official Ultimate Software also shows no percent — only LED on the mouse.
        // Dock is power via pogo pins; the PC still only sees the adapter.
        let t = self.t();
        let (label, fill, frac, tip) = match self.battery {
            Some(b) if b.known || b.percent > 0 => {
                let fill = if b.charging {
                    OK_GREEN
                } else if b.percent <= 15 {
                    NES_RED
                } else if b.percent <= 35 {
                    Color32::from_rgb(0xe6, 0xa0, 0x2e)
                } else {
                    OK_GREEN
                };
                let suffix = if b.charging { t.charging_suffix } else { "" };
                (
                    format!("{}%{suffix}", b.percent),
                    fill,
                    Some((b.percent as f32 / 100.0).clamp(0.0, 1.0)),
                    String::new(),
                )
            }
            _ => (
                t.battery_led_only.into(),
                MUTED,
                None,
                t.battery_led_tip.into(),
            ),
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new(t.battery).color(CREAM_DIM).size(12.0));
            let bar_w = 56.0_f32;
            let bar_h = 16.0_f32;
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(bar_w + 4.0, bar_h), egui::Sense::hover());
            if !tip.is_empty() {
                response.on_hover_text(&tip);
            }
            let body = egui::Rect::from_min_size(rect.min, Vec2::new(bar_w, bar_h));
            let painter = ui.painter();
            painter.rect(
                body,
                Rounding::same(3.0),
                Color32::from_rgb(0x2a, 0x29, 0x30),
                Stroke::new(1.5_f32, CREAM_DIM),
            );
            let tip_rect = egui::Rect::from_min_size(
                egui::pos2(body.right() + 1.0, body.center().y - 4.0),
                Vec2::new(3.5, 8.0),
            );
            painter.rect_filled(tip_rect, Rounding::same(1.0), CREAM_DIM);
            if let Some(pct) = frac {
                let inner = body.shrink(2.5);
                let fill_w = (inner.width() * pct).max(if pct > 0.0 { 3.0 } else { 0.0 });
                if fill_w > 0.0 {
                    let fill_rect =
                        egui::Rect::from_min_size(inner.min, Vec2::new(fill_w, inner.height()));
                    painter.rect_filled(fill_rect, Rounding::same(2.0), fill);
                }
            } else {
                // LED-only: small dot in the middle so it does not look like 0%.
                let c = body.center();
                painter.circle_filled(c, 2.5, MUTED);
            }
            ui.add_space(6.0);
            let lbl = ui.label(RichText::new(label).color(fill).size(12.0).strong());
            if !tip.is_empty() {
                lbl.on_hover_text(&tip);
            }
        });
    }

    fn action_combo(&mut self, ui: &mut egui::Ui, label: &str, value: &mut Action) {
        let lang = self.lang();
        let row_w = ui.available_width().max(1.0);
        ui.vertical(|ui| {
            ui.set_max_width(row_w);
            ui.add(egui::Label::new(RichText::new(label).color(CREAM_DIM).size(13.0)).wrap());
            ui.add_space(2.0);
            let combo_w = row_w;
            let popup_w = combo_w.clamp(200.0, 340.0);
            let combo_id = ui.make_persistent_id(egui::Id::new(label));
            let response = egui::ComboBox::from_id_salt(label)
                .width(combo_w)
                .height(320.0)
                .selected_text(
                    RichText::new(value.label_for(lang))
                        .color(CREAM)
                        .size(14.0),
                )
                .show_ui(ui, |ui| {
                    // ComboBox already has ScrollArea — avoid set_max_height here,
                    // otherwise items get squashed and text overlaps.
                    ui.set_min_width(popup_w);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.spacing_mut().button_padding = Vec2::new(12.0, 7.0);
                    ui.visuals_mut().widgets.inactive.fg_stroke = Stroke::new(1.0_f32, CREAM);
                    ui.visuals_mut().widgets.hovered.bg_fill =
                        Color32::from_rgb(0x4e, 0x4c, 0x58);
                    ui.visuals_mut().widgets.hovered.fg_stroke =
                        Stroke::new(1.0_f32, Color32::WHITE);
                    ui.visuals_mut().selection.bg_fill = NES_RED.gamma_multiply(0.65);
                    ui.visuals_mut().selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);

                    let mut last_cat = "";
                    for item in ACTION_LIST {
                        let cat = item.id.category_for(lang);
                        if cat != last_cat {
                            if !last_cat.is_empty() {
                                ui.add_space(6.0);
                            }
                            ui.label(
                                RichText::new(cat)
                                    .size(12.0)
                                    .color(Color32::from_rgb(0xff, 0x8a, 0x7a))
                                    .strong(),
                            );
                            last_cat = cat;
                        }
                        let selected = *value == item.id;
                        let item_label = item.id.label_for(lang);
                        let text = RichText::new(item_label)
                            .size(14.0)
                            .color(if selected {
                                Color32::WHITE
                            } else {
                                CREAM
                            });
                        let resp = ui.add_sized(
                            [(popup_w - 12.0).max(120.0), 28.0],
                            egui::SelectableLabel::new(selected, text),
                        );
                        if resp.clicked() {
                            *value = item.id;
                        }
                    }
                });
            if egui::ComboBox::is_open(ui.ctx(), combo_id)
                || response.response.hovered()
                || response.response.has_focus()
            {
                self.combo_open = true;
            }
        });
    }

    fn reset_software(&mut self) {
        let t = self.t();
        self.cfg.buttons.side = Action::Default;
        self.cfg.buttons.extra = Action::Default;
        self.cfg.software_remap = true;
        match save_config(&self.cfg_path, &self.cfg) {
            Ok(()) => {
                if self.cfg.software_remap {
                    match start_daemon(&self.cfg_path) {
                        Ok(()) => {
                            self.message = t.msg_reset_sw_daemon_on.into();
                        }
                        Err(e) => {
                            self.message = format!(
                                "Reset in config file, but background service failed to start: {e}"
                            );
                        }
                    }
                } else {
                    self.message = t.msg_reset_sw.into();
                }
            }
            Err(e) => self.message = format!("{}: {e}", t.msg_reset_fail),
        }
        self.refresh_status();
    }

    fn reset_hardware(&mut self) {
        let t = self.t();
        // Firmware default: rear = Back (5), front = Forward (4).
        match write_side_button_maps(5, 4) {
            Ok(()) => {
                self.cfg.buttons.side = Action::Default;
                self.cfg.buttons.extra = Action::Default;
                self.cfg.saved_to_mouse = true;
                let _ = save_config(&self.cfg_path, &self.cfg);
                self.message = t.msg_reset_hw.into();
            }
            Err(e) => self.message = format!("{}: {e}", t.msg_reset_mouse_fail),
        }
    }

    fn try_save_to_mouse(&mut self) {
        let t = self.t();
        let lang = self.lang();
        let side = match hardware_mouse_value(self.cfg.buttons.side) {
            Some(v) => v,
            None if self.cfg.buttons.side == Action::Default => read_button_map(3).unwrap_or(5),
            None => {
                self.message = action_msg(t.msg_side_needs_daemon, self.cfg.buttons.side, lang);
                return;
            }
        };
        let extra = match hardware_mouse_value(self.cfg.buttons.extra) {
            Some(v) => v,
            None if self.cfg.buttons.extra == Action::Default => read_button_map(4).unwrap_or(4),
            None => {
                self.message = action_msg(t.msg_extra_needs_daemon, self.cfg.buttons.extra, lang);
                return;
            }
        };
        match write_side_button_maps(side, extra) {
            Ok(()) => {
                self.cfg.saved_to_mouse = true;
                let _ = save_config(&self.cfg_path, &self.cfg);
                self.message = t.msg_saved_mouse.into();
            }
            Err(e) => self.message = format!("{}: {e}", t.msg_write_mouse_fail),
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = BG_DEEP;
        style.visuals.extreme_bg_color = BG_DEEP;
        style.visuals.faint_bg_color = CARD;
        style.visuals.widgets.inactive.bg_fill = BG_DEEP;
        style.visuals.widgets.inactive.weak_bg_fill = BG_DEEP;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, CREAM);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, CARD_EDGE);
        style.visuals.widgets.inactive.rounding = Rounding::same(BTN_ROUND);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x4e, 0x4c, 0x58);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x4e, 0x4c, 0x58);
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, CREAM_DIM);
        style.visuals.widgets.hovered.rounding = Rounding::same(BTN_ROUND);
        style.visuals.widgets.active.bg_fill = NES_RED;
        style.visuals.widgets.active.rounding = Rounding::same(BTN_ROUND);
        style.visuals.widgets.open.bg_fill = CARD;
        style.visuals.widgets.open.weak_bg_fill = CARD;
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, CREAM);
        style.visuals.widgets.open.bg_stroke = Stroke::new(1.0, NES_RED);
        style.visuals.widgets.open.rounding = Rounding::same(BTN_ROUND);
        style.visuals.selection.bg_fill = NES_RED.gamma_multiply(0.55);
        style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
        style.visuals.window_rounding = Rounding::same(8.0);
        style.visuals.window_stroke = Stroke::new(1.0, CARD_EDGE);
        style.visuals.menu_rounding = Rounding::same(6.0);
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(16.0, 10.0);
        style.spacing.interact_size = Vec2::new(40.0, 28.0);
        style.spacing.combo_height = 320.0;
        ctx.set_style(style);
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        Self::apply_theme(ctx);
        self.combo_open = false;
        self.pump_poll();
        if self.last_status.elapsed() > Duration::from_secs(2) {
            self.refresh_status();
        }

        let t = self.t();
        let lang = self.lang();

        egui::CentralPanel::default()
            .frame(Frame::default().fill(BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                // ScrollArea fills the window so the bar sits on the right edge.
                // Content padding (incl. right) lives inside the scroll viewport.
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Frame::default()
                            .inner_margin(Margin::same(16.0))
                            .show(ui, |ui| {
                                let outer_w = ui.available_width();
                                let content_w = outer_w.min(CONTENT_MAX_W).max(260.0).min(outer_w);
                                let side_pad = ((outer_w - content_w) * 0.5).max(0.0);

                                ui.horizontal(|ui| {
                                    if side_pad > 0.0 {
                                        ui.add_space(side_pad);
                                    }
                                    ui.vertical(|ui| {
                                        ui.set_width(content_w);
                                        ui.set_max_width(content_w);

                // Header: language first (top-right), then title, status, battery
                ui.allocate_ui_with_layout(
                    Vec2::new(content_w, 28.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        self.language_widget(ui);
                    },
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new("8BitDo R8")
                            .color(CREAM)
                            .size(26.0)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("N Edition")
                            .color(NES_RED)
                            .size(16.0)
                            .strong(),
                    );
                });
                ui.add_space(8.0);
                muted_label(ui, &self.status, 13.0);
                ui.add_space(10.0);
                self.battery_widget(ui);
                ui.add_space(14.0);

                // Side buttons card
                card(ui, |ui| {
                    ui.label(
                        RichText::new(t.side_buttons)
                            .color(CREAM)
                            .size(16.0)
                            .strong(),
                    );
                    muted_label(ui, t.side_buttons_help, 12.5);
                    ui.add_space(8.0);
                    let mut side = self.cfg.buttons.side;
                    let mut extra = self.cfg.buttons.extra;
                    self.action_combo(ui, t.rear_button, &mut side);
                    ui.add_space(4.0);
                    self.action_combo(ui, t.front_button, &mut extra);
                    self.cfg.buttons.side = side;
                    self.cfg.buttons.extra = extra;
                });

                ui.add_space(10.0);

                // DPI card
                card(ui, |ui| {
                    ui.label(
                        RichText::new(t.dpi_title)
                            .color(CREAM)
                            .size(16.0)
                            .strong(),
                    );
                    if let Some(d) = &self.dpi {
                        let name = stage_color_name(lang, d.index);
                        let hex = STAGE_COLOR_HEX
                            .get(d.index)
                            .copied()
                            .unwrap_or("#888888");
                        let col = hex_color(hex);
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} DPI  ·  {}  ·  {} {}/{}",
                                    d.current_dpi,
                                    name,
                                    t.dpi_step,
                                    d.index + 1,
                                    d.stages.len()
                                ))
                                .color(col)
                                .size(20.0)
                                .strong(),
                            )
                            .wrap(),
                        );
                        ui.add_space(8.0);
                        dpi_stage_grid(ui, &d.stages, d.index, lang);
                        ui.add_space(10.0);
                        muted_label(ui, t.dpi_hint, 11.0);
                    } else if !self.dpi_error.is_empty() {
                        ui.label(
                            RichText::new(format!("{}: {}", t.dpi_error, self.dpi_error))
                                .color(NES_RED),
                        );
                    } else {
                        ui.label(RichText::new(t.dpi_reading).color(MUTED));
                    }
                });

                ui.add_space(10.0);

                // Save card
                card(ui, |ui| {
                    ui.label(
                        RichText::new(t.save_title)
                            .color(CREAM)
                            .size(16.0)
                            .strong(),
                    );
                    muted_label(ui, t.save_intro, 12.5);
                    ui.add_space(10.0);

                    ui.label(
                        RichText::new(t.save_a_title)
                            .color(CREAM_DIM)
                            .size(14.0)
                            .strong(),
                    );
                    muted_label(ui, t.save_a_help, 12.0);
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.cfg.software_remap,
                        RichText::new(t.use_daemon).color(CREAM_DIM),
                    );
                    ui.add_space(6.0);
                    button_row(ui, |ui, full| {
                        if primary_button(ui, t.save_start_daemon, full).clicked() {
                            if let Err(e) = save_config(&self.cfg_path, &self.cfg) {
                                self.message = format!("{}: {e}", t.msg_save_fail);
                            } else if self.cfg.software_remap {
                                match start_daemon(&self.cfg_path) {
                                    Ok(()) => {
                                        self.message = t.msg_saved_daemon_on.into();
                                    }
                                    Err(e) => self.message = e,
                                }
                            } else {
                                stop_daemon();
                                self.message = t.msg_saved_daemon_off.into();
                            }
                            self.refresh_status();
                        }
                        if secondary_button(ui, t.stop_daemon, full).clicked() {
                            stop_daemon();
                            self.message = t.msg_daemon_stopped.into();
                            self.refresh_status();
                        }
                    });
                    ui.add_space(6.0);
                    if reset_link(ui, t.reset_software).clicked() {
                        self.confirm = Some(ConfirmKind::ResetSoftware);
                    }
                    muted_label(ui, t.reset_software_help, 11.0);

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(
                        RichText::new(t.save_b_title)
                            .color(CREAM_DIM)
                            .size(14.0)
                            .strong(),
                    );
                    muted_label(ui, t.save_b_help, 12.0);
                    ui.add_space(2.0);
                    muted_label(ui, t.save_b_limits, 11.0);
                    ui.add_space(8.0);
                    button_row(ui, |ui, full| {
                        if primary_button(ui, t.save_to_mouse, full).clicked() {
                            self.try_save_to_mouse();
                        }
                    });
                    ui.add_space(6.0);
                    if reset_link(ui, t.reset_mouse).clicked() {
                        self.confirm = Some(ConfirmKind::ResetHardware);
                    }
                    muted_label(ui, t.reset_mouse_help, 11.0);
                    if self.cfg.saved_to_mouse {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(t.firmware_saved)
                                .color(OK_GREEN)
                                .size(12.0),
                        );
                    }
                });

                if !self.message.is_empty() {
                    ui.add_space(10.0);
                    card(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.message).color(CREAM).size(13.0),
                            )
                            .wrap(),
                        );
                    });
                }

                ui.add_space(8.0);
                ui.collapsing(RichText::new(t.handedness).color(MUTED), |ui| {
                    muted_label(ui, t.handedness_help, 12.0);
                });
                                    }); // vertical content column
                                }); // horizontal centering
                            }); // content padding frame
                    }); // ScrollArea
            });

        // Confirm dialog for reset actions
        if let Some(kind) = self.confirm {
            let t = self.t();
            let (title, body) = match kind {
                ConfirmKind::ResetSoftware => (t.confirm_reset_sw_title, t.confirm_reset_sw_body),
                ConfirmKind::ResetHardware => (t.confirm_reset_hw_title, t.confirm_reset_hw_body),
            };
            let mut open = true;
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.set_max_width(360.0);
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(body).color(CREAM).size(13.0)).wrap());
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if primary_button(ui, t.confirm_yes, false).clicked() {
                            match kind {
                                ConfirmKind::ResetSoftware => self.reset_software(),
                                ConfirmKind::ResetHardware => self.reset_hardware(),
                            }
                            self.confirm = None;
                        }
                        if secondary_button(ui, t.confirm_cancel, false).clicked() {
                            self.confirm = None;
                        }
                    });
                });
            if !open {
                self.confirm = None;
            }
        }

        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([580.0, 860.0])
            .with_min_inner_size([360.0, 420.0])
            .with_title("8BitDo R8 — N Edition"),
        ..Default::default()
    };
    eframe::run_native(
        "8BitDo R8",
        opts,
        Box::new(|cc| {
            AppState::apply_theme(&cc.egui_ctx);
            Ok(Box::new(AppState::new()))
        }),
    )
}
