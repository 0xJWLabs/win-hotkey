# win-hotkey

[![Crates.io](https://img.shields.io/crates/v/windows-hotkeys?style=flat-square)](https://crates.io/crates/windows-hotkeys)
[![Crates.io](https://img.shields.io/crates/l/windows-hotkeys?style=flat-square)](https://crates.io/crates/windows-hotkeys)
[![Docs.rs](https://img.shields.io/docsrs/windows-hotkeys?style=flat-square)](https://docs.rs/windows-hotkeys/latest/win-hotkey)

**`win-hotkey`** is a lightweight and opinionated Rust crate designed for handling system-wide hotkeys on Windows. It provides an easy-to-use abstraction over the Windows API, enabling thread-safe hotkey registration and callback execution.

---

## 🚀 Features

- **Thread-Safe Hotkeys**: Handles Windows API's single-thread limitation seamlessly.
- **High-Level Abstraction**: Simple interface to register and manage hotkeys.
- **Customizable Callbacks**: Assign functions or closures to execute on hotkey triggers.
- **Flexible Key Combination Support**: Register hotkeys with:
  - Modifier + Key (e.g., `Alt + A`)
  - Modifier + Key + Additional Keys
- **Rust-Friendly**: Uses high-level abstractions for Virtual Keys (`VK_*`) and Modifier Keys (`MOD_*`).
- **String-Based Keys**: Create virtual keys (`VirtualKey`) and modifiers keys (`ModifiersKey`) from human-readable strings.

---

## 📖 Usage

### Quick Start

1. Create a `HotkeyManager` instance.
2. Register a hotkey with a `VirtualKey`, one or more `ModifiersKey`s, and a callback.
3. Run the event loop to listen for hotkey triggers.

```rust
use win_hotkey::keys::{ModifiersKey, VirtualKey};
use win_hotkey::{HotkeyManager, HotkeyManagerImpl};

fn main() {
    let mut hkm = HotkeyManager::new();

    hkm.register(VirtualKey::A, &[ModifiersKey::Alt], || {
        println!("Hotkey ALT + A was pressed");
    })
    .unwrap();

    hkm.event_loop();
}
