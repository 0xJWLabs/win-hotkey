#![allow(clippy::uninlined_format_args)]

//! win_hotkey lets you register Global HotKeys for Desktop Applications.
//!
//! # Example
//!
//! ```no_run
//! use win_hotkey::{WinHotKeyManager, hotkey::{HotKey, Modifiers, Code}};
//!
//! // initialize the hotkeys manager
//! let manager = WinHotKeyManager::new().unwrap();
//!
//! // construct the hotkey
//! let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyD);
//!
//! // register it
//! manager.register(hotkey);
//! ```
//!
//!
//! # Processing global hotkey events
//!
//! You can also listen for the menu events using [`WinHotKeyEvent::receiver`] to get events for the hotkey pressed events.
//! ```no_run
//! use win_hotkey::WinHotKeyEvent;
//!
//! if let Ok(event) = WinHotKeyEvent::receiver().try_recv() {
//!     println!("{:?}", event);
//! }
//! ```
mod error;
pub mod hotkey;
mod manager;

use crossbeam_channel::unbounded;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;

pub use self::error::*;
pub use hotkey::HotKey;
pub use manager::WinHotKeyManager;

/// Describes the state of the [`HotKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum HotKeyState {
    /// The [`HotKey`] is pressed (the key is down).
    Pressed,
    /// The [`HotKey`] is released (the key is up).
    Released,
}

/// Describes a global hotkey event emitted when a [`HotKey`] is pressed or released.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct WinHotKeyEvent {
    /// Id of the associated [`HotKey`].
    pub id: u32,

    /// State of the associated [`HotKey`].
    pub state: HotKeyState,
}

/// A reciever that could be used to listen to global hotkey events.
pub type WinHotKeyEventReceiver = Receiver<WinHotKeyEvent>;
type WinHotKeyEventHandler = Box<dyn Fn(WinHotKeyEvent) + Send + Sync + 'static>;

static WIN_HOTKEY_CHANNEL: Lazy<(Sender<WinHotKeyEvent>, WinHotKeyEventReceiver)> =
    Lazy::new(unbounded);

static WIN_HOTKEY_EVENT_HANDLER: OnceCell<Option<WinHotKeyEventHandler>> = OnceCell::new();

impl WinHotKeyEvent {
    /// Returns the id of the associated [`HotKey`].
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Returns the state of the associated [`HotKey`].

    pub fn state(&self) -> HotKeyState {
        self.state
    }

    /// Gets a reference to the event channel's [`WinHotKeyEventReceiver`]
    /// which can be used to listen for global hotkey events.
    ///
    /// ## Note
    ///
    /// This will not receive any events if [`WinHotKeyEvent::set_event_handler`] has been called with a `Some` value.
    pub fn receiver<'a>() -> &'a WinHotKeyEventReceiver {
        &WIN_HOTKEY_CHANNEL.1
    }

    /// Set a handler to be called for new events. Useful for implementing custom event sender.
    ///
    /// ## Note
    ///
    /// Calling this function with a `Some` value,
    /// will not send new events to the channel associated with [`WinHotKeyEvent::receiver`]
    pub fn set_event_handler<F: Fn(WinHotKeyEvent) + Send + Sync + 'static>(f: Option<F>) {
        if let Some(f) = f {
            let _ = WIN_HOTKEY_EVENT_HANDLER.set(Some(Box::new(f)));
        } else {
            let _ = WIN_HOTKEY_EVENT_HANDLER.set(None);
        }
    }

    pub(crate) fn send(event: WinHotKeyEvent) {
        if let Some(handler) = WIN_HOTKEY_EVENT_HANDLER.get_or_init(|| None) {
            handler(event);
        } else {
            let _ = WIN_HOTKEY_CHANNEL.0.send(event);
        }
    }
}
