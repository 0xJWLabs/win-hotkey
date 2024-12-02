use thiserror::Error;

use crate::keys::VirtualKey;

#[derive(Debug, Error)]
pub enum HotkeyError {
    #[error("invalid key name `{0}`")]
    InvalidKey(String),
    #[error("invalid key char `{0}`")]
    InvalidKeyChar(char),
    #[error("VKey is not a ModKey `{0}`")]
    NotAModkey(VirtualKey),
    #[error("Hotkey registration failed. Hotkey or Id might be in use already")]
    RegistrationFailed,
    #[error("Hotkey unregistration failed")]
    UnregistrationFailed,
}
