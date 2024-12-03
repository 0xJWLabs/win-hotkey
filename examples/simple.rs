use win_hotkey::keys::ModifiersKey;
use win_hotkey::keys::VirtualKey;
use win_hotkey::single_thread::HotkeyManager;
use win_hotkey::HotkeyManagerImpl;

fn main() {
    // Create a HotkeyManager
    let mut hkm = HotkeyManager::new();

    // Register a system-wide hotkey with the main key `A` and the modifier key `ALT`
    hkm.register(VirtualKey::F8, None, || {
        println!("Hotkey F8 was pressed");
    })
    .unwrap();

    // Register a system-wide hotkey with the main key `A` and the modifier key `ALT`
    hkm.register(VirtualKey::R, Some(&[ModifiersKey::Ctrl]), || {
        println!("Hotkey CTRL + R was pressed");
    })
    .unwrap();

    // Register a system-wide hotkey with the main key `B` and multiple modifier keys
    // (`CTRL` + `ALT`)
    hkm.register(
        VirtualKey::B,
        Some(&[ModifiersKey::Ctrl, ModifiersKey::Alt]),
        || {
            println!("Hotkey CTRL + ALT + B was pressed");
        },
    )
    .unwrap();

    // Run the event handler in a blocking loop. This will block forever and execute the set
    // callbacks when registered hotkeys are detected
    hkm.event_loop();
}
