//! Shared logic for 8BitDo Retro R8: config, HID protocol, actions, i18n.

mod actions;
mod config;
mod hid;
mod i18n;

pub use actions::{hardware_mouse_value, Action, ACTION_LIST};
pub use config::{default_config_dir, load_config, save_config, Config, CONFIG_NAME};
pub use hid::{
    find_config_hidraw, read_battery, read_button_map, read_dpi, write_button_map,
    write_side_button_maps, BatteryState, DpiState, HidError, DEFAULT_DPI_STAGES, STAGE_COLORS,
    STAGE_COLOR_HEX,
};
pub use i18n::{
    action_category, action_label, stage_color_name, ui_text, Lang, UiText,
};
