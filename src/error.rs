use crate::keys::VirtualKey;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result;

pub enum HotkeyError {
    InvalidKey(String),
    InvalidKeyChar(char),
    NotAModkey(VirtualKey),
    RegistrationFailed,
    UnregistrationFailed,
}

impl Display for HotkeyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match *self {
            HotkeyError::InvalidKey(ref key) => write!(f, "invalid key name `{}`", key),
            HotkeyError::InvalidKeyChar(ref ch) => write!(f, "invalid key char `{}`", ch),
            HotkeyError::NotAModkey(ref vkey) => write!(f, "VKey is not a ModKey {:?}", vkey),
            HotkeyError::RegistrationFailed => write!(
                f,
                "Hotkey registration failed. Hotkey or Id might be in use already"
            ),
            HotkeyError::UnregistrationFailed => write!(f, "Hotkey unregistration failed"),
        }
    }
}

impl Debug for HotkeyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match *self {
            HotkeyError::InvalidKey(ref key) => write!(f, "invalid key name `{}`", key),
            HotkeyError::InvalidKeyChar(ref ch) => write!(f, "invalid key char `{}`", ch),
            HotkeyError::NotAModkey(ref vkey) => write!(f, "VKey is not a ModKey {:?}", vkey),
            HotkeyError::RegistrationFailed => write!(
                f,
                "Hotkey registration failed. Hotkey or Id might be in use already"
            ),
            HotkeyError::UnregistrationFailed => write!(f, "Hotkey unregistration failed"),
        }
    }
}

impl Error for HotkeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
