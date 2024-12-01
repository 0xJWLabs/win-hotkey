use crate::hotkey::HotKey;
use crate::WinHotKeyEvent;
use keyboard_types::Code;
use keyboard_types::Modifiers;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ptr;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::ERROR_HOTKEY_ALREADY_REGISTERED;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::LPARAM;
use windows_sys::Win32::Foundation::LRESULT;
use windows_sys::Win32::Foundation::WIN32_ERROR;
use windows_sys::Win32::Foundation::WPARAM;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW;
use windows_sys::Win32::UI::WindowsAndMessaging::DefWindowProcW;
use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassW;
use windows_sys::Win32::UI::WindowsAndMessaging::CW_USEDEFAULT;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
use windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSW;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TRANSPARENT;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED;

pub struct SendHWND(HWND);
unsafe impl Send for SendHWND {}

pub static HOTKEYS: Lazy<Mutex<HashMap<u32, Option<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub struct WinHotKeyManager {
    hwnd: SendHWND,
}

impl Drop for WinHotKeyManager {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd.0) };
    }
}

impl WinHotKeyManager {
    pub fn new() -> crate::Result<Self> {
        let class_name = encode_wide("win_hotkey_app");
        unsafe {
            let hinstance = get_instance_handle();

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(win_hotkey_proc),
                lpszClassName: class_name.as_ptr(),
                hInstance: hinstance,
                ..std::mem::zeroed()
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED |
                // WS_EX_TOOLWINDOW prevents this window from ever showing up in the taskbar, which
                // we want to avoid. If you remove this style, this window won't show up in the
                // taskbar *initially*, but it can show up at some later point. This can sometimes
                // happen on its own after several hours have passed, although this has proven
                // difficult to reproduce. Alternatively, it can be manually triggered by killing
                // `explorer.exe` and then starting the process back up.
                // It is unclear why the bug is triggered by waiting for several hours.
                WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                ptr::null(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                return Err(crate::Error::OsError(std::io::Error::last_os_error()));
            }

            Ok(Self {
                hwnd: SendHWND(hwnd),
            })
        }
    }

    pub fn register(&mut self, hotkey: HotKey) -> crate::Result<()> {
        let mut mods = MOD_NOREPEAT;
        if hotkey.mods.contains(Modifiers::SHIFT) {
            mods |= MOD_SHIFT;
        }
        if hotkey.mods.intersects(Modifiers::SUPER | Modifiers::META) {
            mods |= MOD_WIN;
        }
        if hotkey.mods.contains(Modifiers::ALT) {
            mods |= MOD_ALT;
        }
        if hotkey.mods.contains(Modifiers::CONTROL) {
            mods |= MOD_CONTROL;
        }

        // get key scan code
        match key_to_vk(&hotkey.key) {
            Some(vk_code) => {
                let result =
                    unsafe { RegisterHotKey(self.hwnd.0, hotkey.id() as _, mods, vk_code as _) };
                add_hotkey(hotkey.id(), hotkey.name());
                if result == 0 {
                    let error = std::io::Error::last_os_error();

                    return match error.raw_os_error() {
                        Some(raw_os_error) => {
                            let win32error = WIN32_ERROR::try_from(raw_os_error);
                            if let Ok(ERROR_HOTKEY_ALREADY_REGISTERED) = win32error {
                                Err(crate::Error::AlreadyRegistered(hotkey))
                            } else {
                                Err(crate::Error::OsError(error))
                            }
                        }
                        _ => Err(crate::Error::OsError(error)),
                    };
                }
            }
            _ => {
                return Err(crate::Error::FailedToRegister(format!(
                    "Unable to register hotkey (unknown VKCode for this key: {}).",
                    hotkey.key
                )))
            }
        }

        Ok(())
    }

    pub fn unregister(&self, hotkey: HotKey) -> crate::Result<()> {
        let result = unsafe { UnregisterHotKey(self.hwnd.0, hotkey.id() as _) };
        remove_hotkey(hotkey.id());
        if result == 0 {
            return Err(crate::Error::FailedToUnRegister(hotkey));
        }
        Ok(())
    }

    pub fn register_all(&mut self, hotkeys: &[HotKey]) -> crate::Result<()> {
        for hotkey in hotkeys {
            self.register(hotkey.clone())?;
        }
        Ok(())
    }

    pub fn unregister_all(&self, hotkeys: &[HotKey]) -> crate::Result<()> {
        for hotkey in hotkeys {
            self.unregister(hotkey.clone())?;
        }
        Ok(())
    }
}

unsafe extern "system" fn win_hotkey_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_HOTKEY {
        let hotkeys = &*HOTKEYS.lock().unwrap();
        let name = hotkeys.get(&(wparam as _)).and_then(|n| n.clone());

        let _ = hotkeys;
        WinHotKeyEvent::send(WinHotKeyEvent {
            id: wparam as _,
            state: crate::HotKeyState::Pressed,
            name: name.clone(),
        });
        std::thread::spawn(move || loop {
            let state = GetAsyncKeyState(HIWORD(lparam as u32) as i32);
            if state == 0 {
                WinHotKeyEvent::send(WinHotKeyEvent {
                    id: wparam as _,
                    state: crate::HotKeyState::Released,
                    name: name.clone(),
                });
                break;
            }
        });
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[inline(always)]
#[allow(non_snake_case)]
const fn HIWORD(x: u32) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

pub fn encode_wide<S: AsRef<std::ffi::OsStr>>(string: S) -> Vec<u16> {
    std::os::windows::prelude::OsStrExt::encode_wide(string.as_ref())
        .chain(std::iter::once(0))
        .collect()
}

fn add_hotkey(key: u32, value: Option<String>) {
    let mut hotkeys = HOTKEYS.lock().unwrap();
    hotkeys.insert(key, value);

    drop(hotkeys);
}

fn remove_hotkey(key: u32) {
    let mut hotkeys = HOTKEYS.lock().unwrap();

    if hotkeys.contains_key(&key) {
        hotkeys.remove(&key);
    }
}

pub fn get_instance_handle() -> windows_sys::Win32::Foundation::HMODULE {
    // Gets the instance handle by taking the address of the
    // pseudo-variable created by the microsoft linker:
    // https://devblogs.microsoft.com/oldnewthing/20041025-00/?p=37483

    // This is preferred over GetModuleHandle(NULL) because it also works in DLLs:
    // https://stackoverflow.com/questions/21718027/getmodulehandlenull-vs-hinstance

    extern "C" {
        static __ImageBase: windows_sys::Win32::System::SystemServices::IMAGE_DOS_HEADER;
    }

    unsafe { &__ImageBase as *const _ as _ }
}

// used to build accelerators table from Key
fn key_to_vk(key: &Code) -> Option<VIRTUAL_KEY> {
    Some(match key {
        Code::KeyA => VK_A,
        Code::KeyB => VK_B,
        Code::KeyC => VK_C,
        Code::KeyD => VK_D,
        Code::KeyE => VK_E,
        Code::KeyF => VK_F,
        Code::KeyG => VK_G,
        Code::KeyH => VK_H,
        Code::KeyI => VK_I,
        Code::KeyJ => VK_J,
        Code::KeyK => VK_K,
        Code::KeyL => VK_L,
        Code::KeyM => VK_M,
        Code::KeyN => VK_N,
        Code::KeyO => VK_O,
        Code::KeyP => VK_P,
        Code::KeyQ => VK_Q,
        Code::KeyR => VK_R,
        Code::KeyS => VK_S,
        Code::KeyT => VK_T,
        Code::KeyU => VK_U,
        Code::KeyV => VK_V,
        Code::KeyW => VK_W,
        Code::KeyX => VK_X,
        Code::KeyY => VK_Y,
        Code::KeyZ => VK_Z,
        Code::Digit0 => VK_0,
        Code::Digit1 => VK_1,
        Code::Digit2 => VK_2,
        Code::Digit3 => VK_3,
        Code::Digit4 => VK_4,
        Code::Digit5 => VK_5,
        Code::Digit6 => VK_6,
        Code::Digit7 => VK_7,
        Code::Digit8 => VK_8,
        Code::Digit9 => VK_9,
        Code::Equal => VK_OEM_PLUS,
        Code::Comma => VK_OEM_COMMA,
        Code::Minus => VK_OEM_MINUS,
        Code::Period => VK_OEM_PERIOD,
        Code::Semicolon => VK_OEM_1,
        Code::Slash => VK_OEM_2,
        Code::Backquote => VK_OEM_3,
        Code::BracketLeft => VK_OEM_4,
        Code::Backslash => VK_OEM_5,
        Code::BracketRight => VK_OEM_6,
        Code::Quote => VK_OEM_7,
        Code::Backspace => VK_BACK,
        Code::Tab => VK_TAB,
        Code::Space => VK_SPACE,
        Code::Enter => VK_RETURN,
        Code::CapsLock => VK_CAPITAL,
        Code::Escape => VK_ESCAPE,
        Code::PageUp => VK_PRIOR,
        Code::PageDown => VK_NEXT,
        Code::End => VK_END,
        Code::Home => VK_HOME,
        Code::ArrowLeft => VK_LEFT,
        Code::ArrowUp => VK_UP,
        Code::ArrowRight => VK_RIGHT,
        Code::ArrowDown => VK_DOWN,
        Code::PrintScreen => VK_SNAPSHOT,
        Code::Insert => VK_INSERT,
        Code::Delete => VK_DELETE,
        Code::F1 => VK_F1,
        Code::F2 => VK_F2,
        Code::F3 => VK_F3,
        Code::F4 => VK_F4,
        Code::F5 => VK_F5,
        Code::F6 => VK_F6,
        Code::F7 => VK_F7,
        Code::F8 => VK_F8,
        Code::F9 => VK_F9,
        Code::F10 => VK_F10,
        Code::F11 => VK_F11,
        Code::F12 => VK_F12,
        Code::F13 => VK_F13,
        Code::F14 => VK_F14,
        Code::F15 => VK_F15,
        Code::F16 => VK_F16,
        Code::F17 => VK_F17,
        Code::F18 => VK_F18,
        Code::F19 => VK_F19,
        Code::F20 => VK_F20,
        Code::F21 => VK_F21,
        Code::F22 => VK_F22,
        Code::F23 => VK_F23,
        Code::F24 => VK_F24,
        Code::NumLock => VK_NUMLOCK,
        Code::Numpad0 => VK_NUMPAD0,
        Code::Numpad1 => VK_NUMPAD1,
        Code::Numpad2 => VK_NUMPAD2,
        Code::Numpad3 => VK_NUMPAD3,
        Code::Numpad4 => VK_NUMPAD4,
        Code::Numpad5 => VK_NUMPAD5,
        Code::Numpad6 => VK_NUMPAD6,
        Code::Numpad7 => VK_NUMPAD7,
        Code::Numpad8 => VK_NUMPAD8,
        Code::Numpad9 => VK_NUMPAD9,
        Code::NumpadAdd => VK_ADD,
        Code::NumpadDecimal => VK_DECIMAL,
        Code::NumpadDivide => VK_DIVIDE,
        Code::NumpadEnter => VK_RETURN,
        Code::NumpadEqual => VK_E,
        Code::NumpadMultiply => VK_MULTIPLY,
        Code::NumpadSubtract => VK_SUBTRACT,
        Code::ScrollLock => VK_SCROLL,
        Code::AudioVolumeDown => VK_VOLUME_DOWN,
        Code::AudioVolumeUp => VK_VOLUME_UP,
        Code::AudioVolumeMute => VK_VOLUME_MUTE,
        Code::MediaPlay => VK_PLAY,
        Code::MediaPause => VK_PAUSE,
        Code::MediaPlayPause => VK_MEDIA_PLAY_PAUSE,
        Code::MediaStop => VK_MEDIA_STOP,
        Code::MediaTrackNext => VK_MEDIA_NEXT_TRACK,
        Code::MediaTrackPrevious => VK_MEDIA_PREV_TRACK,
        Code::Pause => VK_PAUSE,
        _ => return None,
    })
}
