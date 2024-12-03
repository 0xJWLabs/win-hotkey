use std::marker::PhantomData;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread::spawn;
use std::thread::JoinHandle;

use crate::error::HotkeyError;
use crate::keys::ModifiersKey;
use crate::keys::VirtualKey;
use crate::single_thread;
use crate::HotkeyId;
use crate::HotkeyManagerImpl;
use crate::InterruptHandle;

pub struct Hotkey<T: 'static> {
    virtual_key: VirtualKey,
    modifiers_key: Option<Vec<ModifiersKey>>,
    extra_keys: Option<Vec<VirtualKey>>,
    callback: Option<Box<dyn Fn() -> T + Send + 'static>>,
}

enum HotkeyMessage<T: 'static> {
    Register(Sender<Result<HotkeyId, HotkeyError>>, Hotkey<T>),
    HandleHotkey(Sender<Option<T>>),
    Unregister(Sender<Result<(), HotkeyError>>, HotkeyId),
    UnregisterAll(Sender<Result<(), HotkeyError>>),
    EventLoop(Sender<()>),
    InterruptHandle(Sender<InterruptHandle>),
    Exit(Sender<()>),
}

pub struct HotkeyManager<T: 'static> {
    no_repeat: bool,
    _phantom: PhantomData<T>,
    sender: Sender<HotkeyMessage<T>>,
    backend_handle: Option<JoinHandle<()>>,
}

struct TSHotkeyManagerBackend<T: 'static> {
    hkm: single_thread::HotkeyManager<T>,
    receiver: Receiver<HotkeyMessage<T>>,
}

impl<T: 'static> HotkeyManager<T> {
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

impl<T> TSHotkeyManagerBackend<T> {
    /// Create a new HotkeyManager instance. To work around the same-thread limitation of the
    /// windows event API, this will launch a new background thread to handle hotkey interactions.
    ///
    fn new(receiver: Receiver<HotkeyMessage<T>>) -> Self {
        let mut hkm = single_thread::HotkeyManager::new();
        hkm.set_no_repeat(false);
        Self { hkm, receiver }
    }

    fn backend_loop(&mut self) {
        while let Ok(msg) = self.receiver.recv() {
            match msg {
                HotkeyMessage::Register(channel, hotkey) => {
                    let return_value = self.hkm.register_extrakeys(
                        hotkey.virtual_key,
                        hotkey.modifiers_key.as_deref(),
                        hotkey.extra_keys.as_deref(),
                        hotkey.callback,
                    );
                    channel.send(return_value).unwrap();
                }
                HotkeyMessage::HandleHotkey(channel) => {
                    let return_value = self.hkm.handle_hotkey();
                    channel.send(return_value).unwrap();
                }
                HotkeyMessage::Unregister(channel, hotkey_id) => {
                    let return_value = self.hkm.unregister(hotkey_id);
                    channel.send(return_value).unwrap();
                }
                HotkeyMessage::UnregisterAll(channel) => {
                    let return_value = self.hkm.unregister_all();
                    channel.send(return_value).unwrap();
                }
                HotkeyMessage::EventLoop(channel) => {
                    self.hkm.event_loop();
                    channel.send(()).unwrap();
                }
                HotkeyMessage::InterruptHandle(channel) => {
                    let return_value = self.hkm.interrupt_handle();
                    channel.send(return_value).unwrap();
                }
                HotkeyMessage::Exit(channel) => {
                    channel.send(()).unwrap();
                    return;
                }
            }
        }
    }
}

impl<T: 'static + Send> HotkeyManagerImpl<T> for HotkeyManager<T> {
    fn new() -> Self {
        let (sender, receiver) = channel();
        let backend_handle = spawn(move || {
            let mut backend = TSHotkeyManagerBackend::<T>::new(receiver);
            backend.backend_loop();
        });
        Self {
            no_repeat: true,
            _phantom: PhantomData,
            sender,
            backend_handle: Some(backend_handle),
        }
    }

    fn register_extrakeys(
        &mut self,
        virtual_key: VirtualKey,
        modifiers_key: Option<&[ModifiersKey]>,
        extra_keys: Option<&[VirtualKey]>,
        callback: Option<impl Fn() -> T + Send + 'static>,
    ) -> Result<HotkeyId, HotkeyError> {
        let return_channel = channel();

        let mut modifiers_key = modifiers_key.map(|keys| keys.to_vec());

        if self.no_repeat {
            modifiers_key
                .get_or_insert_with(Vec::new)
                .push(ModifiersKey::NoRepeat);
        }

        let callback_boxed = callback.map(|cb| Box::new(cb) as Box<dyn Fn() -> T + Send>);

        let hotkey = Hotkey {
            virtual_key,
            modifiers_key,
            extra_keys: extra_keys.map(|keys| keys.to_vec()),
            callback: callback_boxed,
        };
        self.sender
            .send(HotkeyMessage::Register(return_channel.0, hotkey))
            .unwrap();
        return_channel.1.recv().unwrap()
    }

    fn register(
        &mut self,
        virtual_key: VirtualKey,
        modifiers_key: Option<&[ModifiersKey]>,
        callback: Option<impl Fn() -> T + Send + 'static>,
    ) -> Result<HotkeyId, HotkeyError> {
        self.register_extrakeys(virtual_key, modifiers_key, None, callback)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::Unregister(return_channel.0, id))
            .unwrap();
        return_channel.1.recv().unwrap()
    }

    fn unregister_all(&mut self) -> Result<(), HotkeyError> {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::UnregisterAll(return_channel.0))
            .unwrap();
        return_channel.1.recv().unwrap()
    }

    fn handle_hotkey(&self) -> Option<T> {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::HandleHotkey(return_channel.0))
            .unwrap();
        return_channel.1.recv().unwrap()
    }

    fn event_loop(&self) {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::EventLoop(return_channel.0))
            .unwrap();
        return_channel.1.recv().unwrap()
    }

    fn interrupt_handle(&self) -> InterruptHandle {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::InterruptHandle(return_channel.0))
            .unwrap();
        return_channel.1.recv().unwrap()
    }
}

impl<T> Drop for HotkeyManager<T> {
    fn drop(&mut self) {
        let return_channel = channel();
        self.sender
            .send(HotkeyMessage::Exit(return_channel.0))
            .unwrap();
        return_channel.1.recv().unwrap();
        self.backend_handle.take().unwrap().join().unwrap();
    }
}
