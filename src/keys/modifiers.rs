use super::VirtualKey;
use crate::error::HotkeyError;
use std::fmt::Display;

/// Modifier Key for hotkeys.
///
/// See: `fsModifiers` from <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey>
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifiersKey {
    Alt,
    Ctrl,
    Shift,
    Win,
    /// This is a virtual modifier key that is used to prevent automatically repeating triggers
    /// when the hotkey is being held down. When converting to a VirtualKey, this is mapped to KeyCode 0
    NoRepeat,
    Non,
}

impl TryFrom<&str> for ModifiersKey {
    type Error = HotkeyError;

    fn try_from(val: &str) -> Result<Self, Self::Error> {
        Self::from_keyname(val)
    }
}

impl ModifiersKey {
    /// Take in a string and interpret it as one of the modifier keys.
    /// Possible values are:
    /// - ALT
    /// - CTRL / CONTROL
    /// - SHIFT
    /// - WIN / WINDOWS / SUPER
    /// - NOREPEAT / NO_REPEAT
    ///
    pub fn from_keyname(val: &str) -> Result<Self, HotkeyError> {
        Ok(match val.to_ascii_uppercase().as_ref() {
            "ALT" => ModifiersKey::Alt,
            "CTRL" | "CONTROL" => ModifiersKey::Ctrl,
            "SHIFT" => ModifiersKey::Shift,
            "WIN" | "WINDOWS" | "SUPER" => ModifiersKey::Win,
            "NOREPEAT" | "NO_REPEAT" => ModifiersKey::NoRepeat,
            "NON" => ModifiersKey::Non,
            val => return Err(HotkeyError::InvalidKey(val.to_string())),
        })
    }

    /// Obtain the modifier code for the `ModifiersKey`.
    ///
    /// See: `fsModifiers` from <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey>
    ///
    pub const fn to_mod_code(&self) -> u32 {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

        match self {
            ModifiersKey::Alt => MOD_ALT,
            ModifiersKey::Ctrl => MOD_CONTROL,
            ModifiersKey::Shift => MOD_SHIFT,
            ModifiersKey::Win => MOD_WIN,
            ModifiersKey::NoRepeat => MOD_NOREPEAT,
            ModifiersKey::Non => 0,
        }
    }

    /// Combine multiple `ModifiersKey`s using bitwise OR
    ///
    pub(crate) fn combine(keys: Option<&[ModifiersKey]>) -> u32 {
        if let Some(keys) = keys {
            keys.iter().fold(0, |a, b| a | b.to_mod_code())
        } else {
            ModifiersKey::Non.to_mod_code()
        }
    }
}

impl Display for ModifiersKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = match self {
            ModifiersKey::Alt => "ALT",
            ModifiersKey::Ctrl => "CONTROL",
            ModifiersKey::Shift => "SHIFT",
            ModifiersKey::Win => "WIN",
            ModifiersKey::NoRepeat => "NO_REPEAT",
            ModifiersKey::Non => "NON",
        };
        write!(f, "{}", key)
    }
}

impl From<ModifiersKey> for VirtualKey {
    fn from(mk: ModifiersKey) -> VirtualKey {
        match mk {
            ModifiersKey::Alt => VirtualKey::Menu,
            ModifiersKey::Ctrl => VirtualKey::Control,
            ModifiersKey::Shift => VirtualKey::Shift,
            ModifiersKey::Win => VirtualKey::LWin,
            ModifiersKey::NoRepeat | ModifiersKey::Non => VirtualKey::CustomKeyCode(0),
        }
    }
}
