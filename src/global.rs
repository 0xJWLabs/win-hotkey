use rustc_hash::FxHashMap;

use crate::{HotkeyManager, HotkeyManagerImpl, ModifiersKey, VirtualKey};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct GlobalHotkey<T> {
    key: VirtualKey,
    modifiers: Option<Vec<ModifiersKey>>,
    extras: Option<Vec<VirtualKey>>,
    action: Option<Arc<Mutex<dyn Fn() -> T + Send + 'static>>>, // Callback needs to be Send too
}

pub struct GlobalHotkeyManager<T: Send + 'static> {
    hotkeys: Arc<Mutex<FxHashMap<String, GlobalHotkey<T>>>>,
    manager: Arc<Mutex<HotkeyManager<T>>>,
    listening: Arc<AtomicBool>,
}

impl<T: Send + 'static> GlobalHotkey<T> {
    pub fn set_action(&mut self, action: impl Fn() -> T + Send + 'static) {
        self.action = Some(Arc::new(Mutex::new(action)));
    }
}

impl<T: Send + 'static> Default for GlobalHotkeyManager<T> {
    fn default() -> Self {
        let mut hkm = HotkeyManager::new();
        hkm.set_no_repeat(false);
        Self {
            manager: Arc::new(Mutex::new(hkm)),
            listening: Arc::new(AtomicBool::new(false)),
            hotkeys: Arc::new(Mutex::new(FxHashMap::default())),
        }
    }
}

pub trait GlobalHotkeyManagerImpl<T> {
    fn new() -> Self;
    fn register_hotkey(
        &self,
        name: String,
        key: VirtualKey,
        modifiers: Option<Vec<ModifiersKey>>,
        extras: Option<Vec<VirtualKey>>,
        callback: Option<impl Fn() -> T + Send + 'static>,
    );
    fn add_hotkey(&self, name: String, hotkey: GlobalHotkey<T>);
    fn remove_hotkey(&self, name: String) -> Option<GlobalHotkey<T>>;
    fn start(&self);
    fn stop(&self) -> bool;
}

impl<T: Send + 'static> GlobalHotkeyManagerImpl<T> for GlobalHotkeyManager<T> {
    fn new() -> Self {
        Self::default()
    }

    fn register_hotkey(
        &self,
        name: String,
        key: VirtualKey,
        modifiers: Option<Vec<ModifiersKey>>,
        extras: Option<Vec<VirtualKey>>,
        callback: Option<impl Fn() -> T + Send + 'static>,
    ) {
        let mut hotkeys = self.hotkeys.lock().unwrap();
        hotkeys.insert(
            name,
            GlobalHotkey {
                key,
                modifiers,
                extras,
                action: callback.map(|cb| {
                    Arc::new(Mutex::new(cb)) as Arc<Mutex<dyn Fn() -> T + Send + 'static>>
                }),
            },
        );
    }

    fn add_hotkey(&self, name: String, hotkey: GlobalHotkey<T>) {
        let mut hotkeys = self.hotkeys.lock().unwrap();
        hotkeys.insert(name, hotkey);
    }

    fn remove_hotkey(&self, key: String) -> Option<GlobalHotkey<T>> {
        let mut hotkeys = self.hotkeys.lock().unwrap();
        hotkeys.remove(&key)
    }

    fn start(&self) {
        if self.listening.load(Ordering::SeqCst) {
            eprintln!("Already listening for hotkeys.");
            return;
        }

        let hotkeys = self.hotkeys.clone();
        let hotkey_manager = self.manager.clone();
        let listening = self.listening.clone();

        listening.store(true, Ordering::SeqCst);

        std::thread::spawn(move || {
            // Lock bindings to access keybindings
            let mut hotkey_manager = hotkey_manager.lock().unwrap();
            let hotkeys = hotkeys.lock().unwrap();

            // Collect hotkeys and their actions upfront
            for hotkey in hotkeys.values() {
                let action = hotkey.action.clone();
                let result = if let Some(action) = action {
                    // Register with an action if present
                    hotkey_manager.register_extrakeys(
                        hotkey.key,
                        hotkey.modifiers.as_deref(),
                        hotkey.extras.as_deref(),
                        Some(move || {
                            let action = action.clone();
                            let action = action.lock().unwrap();
                            action()
                        }),
                    )
                } else {
                    // Register without an action if None
                    hotkey_manager.register_extrakeys(
                        hotkey.key,
                        hotkey.modifiers.as_deref(),
                        hotkey.extras.as_deref(),
                        None::<fn() -> T>,
                    )
                };

                if let Err(e) = result {
                    eprintln!("Failed to register keybinding {:?}: {:?}", hotkey.key, e);
                }
            }

            while listening.load(Ordering::SeqCst) {
                hotkey_manager.event_loop();
            }
        });
    }

    fn stop(&self) -> bool {
        if !self.listening.load(Ordering::SeqCst) {
            return false;
        }

        // Set listening flag to false to stop the loop
        self.listening.store(false, Ordering::SeqCst);
        true
    }
}

#[derive(Debug)]
pub enum HotKeyParseError {
    UnsupportedKey(String),
    EmptyToken(String),
    InvalidFormat(String),
}

impl std::fmt::Display for HotKeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            HotKeyParseError::UnsupportedKey(ref key) => {
                write!(
                    f,
                    "Couldn't recognize \"{}\" as a valid key for hotkey",
                    key
                )
            }
            HotKeyParseError::EmptyToken(ref token) => {
                write!(f, "Found empty token while parsing hotkey: {}", token)
            }
            HotKeyParseError::InvalidFormat(ref format) => {
                write!(
                    f,
                    "Invalid hotkey format: \"{}\", a hotkey should have the modifiers first and only one main key, for example: \"Shift + Alt + K\"",
                    format
                )
            }
        }
    }
}

impl std::error::Error for HotKeyParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // No underlying error, so we return None.
        None
    }
}

impl<T: Send + 'static> TryInto<GlobalHotkey<T>> for &str {
    type Error = HotKeyParseError;

    fn try_into(self) -> Result<GlobalHotkey<T>, Self::Error> {
        let tokens = self.split('+').collect::<Vec<&str>>();
        let mut modifiers: Vec<ModifiersKey> = Vec::new();
        let mut key = None;
        let mut extras: Vec<VirtualKey> = Vec::new();

        match tokens.len() {
            1 => {
                // Only a key, no modifiers or extras
                key = Some(
                    VirtualKey::try_from(tokens[0].trim())
                        .map_err(|e| HotKeyParseError::UnsupportedKey(e.to_string()))?,
                );
            }
            _ => {
                let mut found_key = false;

                for raw in tokens {
                    let token = raw.trim();

                    if token.is_empty() {
                        return Err(HotKeyParseError::EmptyToken(self.to_string()));
                    }

                    // If we have already found the key, treat the rest as extras
                    if found_key {
                        let extra_key = VirtualKey::try_from(token)
                            .map_err(|e| HotKeyParseError::UnsupportedKey(e.to_string()))?;
                        extras.push(extra_key);
                    } else {
                        if key.is_some() {
                            return Err(HotKeyParseError::InvalidFormat(self.to_string()));
                        }

                        let temp_key = VirtualKey::try_from(token)
                            .map_err(|e| HotKeyParseError::UnsupportedKey(e.to_string()))?;

                        // If the token is a valid modifier, add it to the modifiers
                        if let Ok(modifier) = temp_key.try_into() {
                            modifiers.push(modifier);
                        } else {
                            // Otherwise, treat it as the main key
                            key = Some(temp_key);
                            found_key = true; // Mark that the key has been found
                        }
                    }
                }
            }
        }

        // If no key was found, return an error
        let key = key.ok_or_else(|| HotKeyParseError::InvalidFormat(self.to_string()))?;

        Ok(GlobalHotkey {
            key,
            modifiers: if modifiers.is_empty() {
                None
            } else {
                Some(modifiers)
            },
            extras: if extras.is_empty() {
                None
            } else {
                Some(extras)
            },
            action: None, // action is still None
        })
    }
}
