use serde::{Deserialize, Serialize};

use crate::i18n::{action_category, action_label, Lang};

/// Actions that can be mapped to side buttons (software remap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    Default,
    Disabled,
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Copy,
    Paste,
    Cut,
    Undo,
    VolUp,
    VolDown,
    Mute,
    PlayPause,
    NextTrack,
    PrevTrack,
    NewTab,
    CloseTab,
    Enter,
    Escape,
}

pub struct ActionItem {
    pub id: Action,
}

/// Ordered list for combo boxes (labels come from i18n).
pub const ACTION_LIST: &[ActionItem] = &[
    ActionItem { id: Action::Default },
    ActionItem { id: Action::Disabled },
    ActionItem { id: Action::Left },
    ActionItem { id: Action::Right },
    ActionItem { id: Action::Middle },
    ActionItem { id: Action::Back },
    ActionItem { id: Action::Forward },
    ActionItem { id: Action::Copy },
    ActionItem { id: Action::Paste },
    ActionItem { id: Action::Cut },
    ActionItem { id: Action::Undo },
    ActionItem { id: Action::VolUp },
    ActionItem { id: Action::VolDown },
    ActionItem { id: Action::Mute },
    ActionItem { id: Action::PlayPause },
    ActionItem { id: Action::NextTrack },
    ActionItem { id: Action::PrevTrack },
    ActionItem { id: Action::NewTab },
    ActionItem { id: Action::CloseTab },
    ActionItem { id: Action::Enter },
    ActionItem { id: Action::Escape },
];

/// Hardware map value in mouse firmware (simple mouse buttons).
/// `None` = cannot be saved in hardware (needs software daemon).
pub fn hardware_mouse_value(action: Action) -> Option<u16> {
    match action {
        Action::Default => None, // keep firmware default
        Action::Disabled => Some(0xff),
        Action::Left => Some(1),
        Action::Right => Some(2),
        Action::Middle => Some(3),
        Action::Forward => Some(4), // BTN_EXTRA
        Action::Back => Some(5),    // BTN_SIDE
        _ => None,
    }
}

impl Action {
    /// English label (default language).
    pub fn label(self) -> &'static str {
        action_label(Lang::En, self)
    }

    pub fn label_for(self, lang: Lang) -> &'static str {
        action_label(lang, self)
    }

    pub fn category_for(self, lang: Lang) -> &'static str {
        action_category(lang, self)
    }
}
