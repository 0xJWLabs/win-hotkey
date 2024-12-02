#[cfg(not(target_os = "windows"))]
compile_error!("Only supported on windows");

use std::collections::HashMap;
use std::marker::PhantomData;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
use windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExA;
use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::GetMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::HWND_MESSAGE;
use windows_sys::Win32::UI::WindowsAndMessaging::MSG;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_HOTKEY;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_NULL;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_DISABLED;
use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE;

use crate::error::HotkeyError;
use crate::get_global_keystate;
use crate::keys::*;
use crate::HotkeyCallback;
use crate::HotkeyId;
use crate::HotkeyManagerImpl;
use crate::InterruptHandle;

struct DropHWND(HWND);

impl Drop for DropHWND {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }
}

pub struct HotkeyManager<T> {
    hwnd: DropHWND,
    id: u16,
    handlers: HashMap<HotkeyId, HotkeyCallback<T>>,
    no_repeat: bool,
    _unimpl_send_sync: PhantomData<*const u8>,
}

impl<T> Default for HotkeyManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HotkeyManager<T> {
    /// Enable or disable the automatically applied `ModKey::NoRepeat` modifier. By default, this
    /// option is set to `true` which causes all hotkey registration calls to add the `NoRepeat`
    /// modifier, thereby disabling automatic retriggers of hotkeys when holding down the keys.
    ///
    /// When this option is disabled, the `ModKey::NoRepeat` can still be manually added while
    /// registering hotkeys.
    ///
    /// Note: Setting this flag doesn't change previously registered hotkeys. It only applies to
    /// registrations performed after calling this function.
    pub fn set_no_repeat(&mut self, no_repeat: bool) {
        self.no_repeat = no_repeat;
    }
}

impl<T> HotkeyManagerImpl<T> for HotkeyManager<T> {
    fn new() -> HotkeyManager<T> {
        let hwnd = create_hidden_window().unwrap_or(DropHWND(std::ptr::null_mut()));
        HotkeyManager {
            hwnd,
            id: 0,
            handlers: HashMap::new(),
            no_repeat: true,
            _unimpl_send_sync: PhantomData,
        }
    }

    fn register_extrakeys(
        &mut self,
        virtual_key: VirtualKey,
        modifiers_key: Option<&[ModifiersKey]>,
        extra_keys: &[VirtualKey],
        callback: impl Fn() -> T + Send + 'static,
    ) -> Result<HotkeyId, HotkeyError> {
        let register_id = HotkeyId(self.id);
        self.id += 1;

        let mut modifiers = ModifiersKey::combine(modifiers_key);
        if self.no_repeat {
            modifiers |= ModifiersKey::NoRepeat.to_mod_code();
        }

        let reg_ok = unsafe {
            RegisterHotKey(
                self.hwnd.0,
                register_id.0 as i32,
                modifiers,
                virtual_key.to_vk_code() as u32,
            )
        };

        if reg_ok == 0 {
            Err(HotkeyError::RegistrationFailed)
        } else {
            // Add the HotkeyCallback to the handlers when the hotkey was registered
            self.handlers.insert(
                register_id,
                HotkeyCallback {
                    callback: Box::new(callback),
                    extra_keys: extra_keys.to_owned(),
                },
            );

            Ok(register_id)
        }
    }

    fn register(
        &mut self,
        virtual_key: VirtualKey,
        modifiers_key: Option<&[ModifiersKey]>,
        callback: impl Fn() -> T + Send + 'static,
    ) -> Result<HotkeyId, HotkeyError> {
        self.register_extrakeys(virtual_key, modifiers_key, &[], callback)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        let ok = unsafe { UnregisterHotKey(self.hwnd.0, id.0 as i32) };

        match ok {
            0 => Err(HotkeyError::UnregistrationFailed),
            _ => {
                self.handlers.remove(&id);
                Ok(())
            }
        }
    }

    fn unregister_all(&mut self) -> Result<(), HotkeyError> {
        let ids: Vec<_> = self.handlers.keys().copied().collect();
        for id in ids {
            self.unregister(id)?;
        }

        Ok(())
    }

    fn handle_hotkey(&self) -> Option<T> {
        loop {
            let mut msg = std::mem::MaybeUninit::<MSG>::uninit();

            // Block and read a message from the message queue. Filtered to receive messages from
            // WM_NULL to WM_HOTKEY
            let ok = unsafe { GetMessageW(msg.as_mut_ptr(), self.hwnd.0, WM_NULL, WM_HOTKEY) };

            if ok != 0 {
                let msg = unsafe { msg.assume_init() };

                if WM_HOTKEY == msg.message {
                    let hk_id = HotkeyId(msg.wParam as u16);

                    // Get the callback for the received ID
                    if let Some(handler) = self.handlers.get(&hk_id) {
                        // Check if all extra keys are pressed
                        if !handler
                            .extra_keys
                            .iter()
                            .any(|vk| !get_global_keystate(*vk))
                        {
                            return Some((handler.callback)());
                        }
                    }
                } else if WM_NULL == msg.message {
                    return None;
                }
            }
        }
    }

    fn event_loop(&self) {
        while self.handle_hotkey().is_some() {}
    }

    fn interrupt_handle(&self) -> InterruptHandle {
        InterruptHandle(self.hwnd.0)
    }
}

impl<T> Drop for HotkeyManager<T> {
    fn drop(&mut self) {
        let _ = self.unregister_all();
    }
}

/// Try to create a hidden "message-only" window
///
fn create_hidden_window() -> Result<DropHWND, ()> {
    let hwnd = unsafe {
        // Get the current module handle
        let hinstance = GetModuleHandleA(std::ptr::null_mut());
        CreateWindowExA(
            WS_EX_NOACTIVATE,
            // The "Static" class is not intended for windows, but this shouldn't matter since the
            // window is hidden anyways
            b"Static\0".as_ptr(),
            b"\0".as_ptr(),
            WS_DISABLED,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        Err(())
    } else {
        Ok(DropHWND(hwnd))
    }
}
