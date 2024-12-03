use rustc_hash::FxHashMap;
use std::sync::Arc;
use win_hotkey::global::GlobalHotkey;
use win_hotkey::global::GlobalHotkeyManager;
use win_hotkey::global::GlobalHotkeyManagerImpl;
use win_hotkey::global::HotKeyParseError;

fn create_binding<T: Send + 'static>(hotkey: &str) -> Result<GlobalHotkey<T>, HotKeyParseError> {
    hotkey.try_into().map_err(|err| {
        eprintln!("Error: {:?}", err);
        err
    })
}

type Hotkeys = Vec<(
    &'static str,
    &'static str,
    Arc<dyn Fn() + Send + Sync + 'static>,
)>;

fn main() {
    let hkm: GlobalHotkeyManager<()> = GlobalHotkeyManager::new();
    let hotkeys: Hotkeys = vec![
        (
            "reload",
            "ctrl+r",
            Arc::new(|| {
                println!("Reload hotkey pressed!");
            }),
        ),
        (
            "reloading",
            "f8",
            Arc::new(|| {
                println!("F8 pressed! Performing action A");
            }),
        ),
    ];

    let mut bindings: FxHashMap<String, Result<GlobalHotkey<()>, HotKeyParseError>> = hotkeys
        .iter()
        .map(|&(action, hotkey_str, _)| (action.to_string(), create_binding::<()>(hotkey_str)))
        .collect();

    for (action, binding_result) in bindings.iter_mut() {
        match binding_result {
            Ok(hotkey) => {
                let callback = hotkeys
                    .iter()
                    .find(|&&(a, _, _)| a == *action)
                    .map(|(_, _, callback)| callback.clone())
                    .unwrap();

                hotkey.set_action(move || callback());
                hkm.add_hotkey(action.to_string(), hotkey.clone());
            }
            Err(e) => {
                // Handle the error for this particular binding
                eprintln!("Error in binding for action '{}': {:?}", action, e);
            }
        }
    }

    std::thread::spawn(move || {
        hkm.start();
    });

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100)); // Sleep for 100 milliseconds
    }
}
