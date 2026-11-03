//! The TSF text input processor: receives keys, runs the shared controller inside edit
//! sessions and mirrors its state into the document's composition.
use crate::{candidate::CandidateWindow, engine, langbar, settings, settings_window};
use keyin::{
    controller::{Controller, Key, KeyEvent, Settings},
    rime::Session,
};
use std::{
    cell::{Cell, RefCell},
    mem::ManuallyDrop,
    rc::Rc,
};
use windows::{
    core::{implement, ComObjectInterface, Interface, Result, BSTR, GUID},
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                GetKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
                KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL,
                VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT,
                VK_LWIN, VK_MENU, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN,
                VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
            },
            TextServices::{
                ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
                ITfContextComposition, ITfDocumentMgr, ITfEditSession, ITfEditSession_Impl,
                ITfFnConfigure, ITfFnConfigure_Impl, ITfFunction, ITfFunction_Impl,
                ITfInsertAtSelection, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr,
                ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemMgr, ITfLangBarItemSink,
                ITfRange, ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx,
                ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr,
                ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, INSERT_TEXT_AT_SELECTION_FLAGS,
                TF_AE_NONE, TF_ANCHOR_END, TF_ANCHOR_START, TF_ES_ASYNCDONTCARE, TF_ES_READWRITE,
                TF_ES_SYNC, TF_IAS_QUERYONLY, TF_LBI_ICON, TF_LBI_TEXT, TF_LBI_TOOLTIP,
                TF_SELECTION, TF_SELECTIONSTYLE,
            },
        },
    },
};

/// State shared by the text service, its edit sessions and the taskbar button.
/// Everything runs on the thread TSF activated the service on.
pub type Shared = Rc<Inner>;

pub struct Inner {
    controller: RefCell<Option<Controller>>,
    composition: RefCell<Option<ITfComposition>>,
    composition_sink: RefCell<Option<ITfCompositionSink>>,
    candidate: RefCell<Option<CandidateWindow>>,
    pub langbar_sink: RefCell<Option<ITfLangBarItemSink>>,
    client_id: Cell<u32>,
    eaten: Cell<bool>,
    replay: Cell<bool>,
}

impl Inner {
    pub fn settings(&self) -> Settings {
        // try_borrow: TSF may call back into the taskbar button during an edit session.
        match self
            .controller
            .try_borrow()
            .ok()
            .and_then(|c| c.as_ref().map(|c| c.settings.clone()))
        {
            Some(settings) => settings,
            None => settings::load(),
        }
    }

    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) {
        let mut updated = self.settings();
        change(&mut updated);
        settings::save(&updated);
        if let Ok(mut controller) = self.controller.try_borrow_mut() {
            if let Some(controller) = controller.as_mut() {
                controller.set_settings(updated);
            }
        }
        self.notify_langbar();
    }

    fn notify_langbar(&self) {
        if let Some(sink) = self.langbar_sink.borrow().as_ref() {
            let _ = unsafe { sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP) };
        }
    }

    fn hide_candidates(&self) {
        if let Some(window) = self.candidate.borrow().as_ref() {
            window.hide();
        }
    }
}

enum Action {
    Key(KeyEvent),
    Commit,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn pressed(key: VIRTUAL_KEY) -> bool {
    (unsafe { GetKeyState(key.0 as i32) }) < 0
}

/// The character a key types on a US QWERTY keyboard. Telex and Pinyin are defined on
/// QWERTY positions, and the Vietnamese profile's own Windows layout would turn the
/// number row into ă â ê…, so KeyIn reads keys itself instead of using the active layout.
fn us_char(vk: u16, shift: bool) -> Option<char> {
    let caps = unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0;
    let pick = |plain: char, shifted: char| Some(if shift { shifted } else { plain });
    match vk {
        0x41..=0x5a => {
            let c = vk as u8 as char;
            Some(if shift != caps {
                c
            } else {
                c.to_ascii_lowercase()
            })
        }
        0x30..=0x39 => {
            let digit = (vk - 0x30) as usize;
            pick(b"0123456789"[digit] as char, b")!@#$%^&*("[digit] as char)
        }
        0x60..=0x69 => Some((b'0' + (vk - 0x60) as u8) as char),
        0x6a => Some('*'),
        0x6b => Some('+'),
        0x6d => Some('-'),
        0x6e => Some('.'),
        0x6f => Some('/'),
        0xba => pick(';', ':'),
        0xbb => pick('=', '+'),
        0xbc => pick(',', '<'),
        0xbd => pick('-', '_'),
        0xbe => pick('.', '>'),
        0xbf => pick('/', '?'),
        0xc0 => pick('`', '~'),
        0xdb => pick('[', '{'),
        0xdc => pick('\\', '|'),
        0xdd => pick(']', '}'),
        0xde => pick('\'', '"'),
        _ => None,
    }
}

/// Translates a virtual key; `None` for modifier keys, which never affect composition.
fn key_event(wparam: WPARAM) -> Option<KeyEvent> {
    let vk = VIRTUAL_KEY(wparam.0 as u16);
    let key = match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU
        | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN | VK_CAPITAL => return None,
        VK_BACK => Key::Backspace,
        VK_ESCAPE => Key::Escape,
        VK_SPACE => Key::Space,
        VK_RETURN => Key::Return,
        VK_TAB => Key::Tab,
        VK_LEFT => Key::Left,
        VK_RIGHT => Key::Right,
        VK_UP => Key::Up,
        VK_DOWN => Key::Down,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        _ => us_char(vk.0, pressed(VK_SHIFT)).map_or(Key::Other, Key::Char),
    };
    Some(KeyEvent {
        key,
        shift: pressed(VK_SHIFT),
        shortcut: pressed(VK_CONTROL) || pressed(VK_MENU),
    })
}

/// Sends a key press again after the composition was committed. Held modifiers stay
/// down, so Ctrl+C after a word still copies.
fn replay(wparam: WPARAM) {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(wparam.0 as u16),
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

#[implement(ITfEditSession)]
struct EditSession {
    shared: Shared,
    context: ITfContext,
    action: Action,
}

impl ITfEditSession_Impl for EditSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let (result, settings_changed) = {
            let Ok(mut controller) = self.shared.controller.try_borrow_mut() else {
                return Ok(());
            };
            let Some(controller) = controller.as_mut() else {
                return Ok(());
            };
            // Keys that finish a word and also mean something to the app (punctuation,
            // Enter, shortcuts) are still eaten: once TSF has claimed a key, legacy apps
            // never see it. The character is typed here, any other key is replayed.
            let mut typed = None;
            let eaten = match self.action {
                Action::Key(event) => {
                    if !controller.handle_key(event) {
                        match event.key {
                            Key::Char(c) if !event.shortcut && !c.is_control() => typed = Some(c),
                            Key::Space if !event.shortcut => typed = Some(' '),
                            _ => self.shared.replay.set(true),
                        }
                    }
                    true
                }
                Action::Commit => {
                    controller.commit_composition();
                    false
                }
            };
            self.shared.eaten.set(eaten);
            let settings_changed = controller.take_settings_changed();
            if settings_changed {
                settings::save(&controller.settings);
            }
            (self.apply(ec, controller, typed), settings_changed)
        };
        // The taskbar reads settings back synchronously, so notify only after the borrow ends.
        if settings_changed {
            self.shared.notify_langbar();
        }
        result
    }
}

impl EditSession {
    fn set_caret(&self, ec: u32, range: &ITfRange, offset: Option<usize>) -> Result<()> {
        let caret = unsafe { range.Clone()? };
        match offset {
            Some(offset) => unsafe {
                caret.Collapse(ec, TF_ANCHOR_START)?;
                let mut moved = 0;
                caret.ShiftStart(ec, offset as i32, &mut moved, std::ptr::null())?;
                caret.Collapse(ec, TF_ANCHOR_START)?;
            },
            None => unsafe { caret.Collapse(ec, TF_ANCHOR_END)? },
        }
        let selection = TF_SELECTION {
            range: ManuallyDrop::new(Some(caret)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_NONE,
                fInterimChar: false.into(),
            },
        };
        let result = unsafe {
            self.context
                .SetSelection(ec, std::slice::from_ref(&selection))
        };
        drop(ManuallyDrop::into_inner(selection.range));
        result
    }

    /// Applies the controller's output: commit first, then the live composition.
    /// `typed` is a character to insert right after the committed word.
    fn apply(&self, ec: u32, controller: &mut Controller, typed: Option<char>) -> Result<()> {
        let mut output = controller.take_output();
        output.commit.extend(typed);
        let mut composition = self.shared.composition.borrow_mut();
        if !output.commit.is_empty() {
            let text = wide(&output.commit);
            if let Some(active) = composition.take() {
                let range = unsafe { active.GetRange()? };
                unsafe { range.SetText(ec, 0, &text)? };
                self.set_caret(ec, &range, None)?;
                unsafe { active.EndComposition(ec)? };
            } else {
                let insert: ITfInsertAtSelection = self.context.cast()?;
                let range = unsafe {
                    insert.InsertTextAtSelection(ec, INSERT_TEXT_AT_SELECTION_FLAGS(0), &text)?
                };
                self.set_caret(ec, &range, None)?;
            }
        }
        if controller.is_composing() {
            if output.composition_changed || composition.is_none() {
                if composition.is_none() {
                    let insert: ITfInsertAtSelection = self.context.cast()?;
                    let range = unsafe { insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])? };
                    let sink = self.shared.composition_sink.borrow().clone();
                    let started = unsafe {
                        self.context
                            .cast::<ITfContextComposition>()?
                            .StartComposition(ec, &range, sink.as_ref())?
                    };
                    *composition = Some(started);
                }
                if let Some(active) = composition.as_ref() {
                    let range = unsafe { active.GetRange()? };
                    unsafe { range.SetText(ec, 0, &wide(controller.preedit()))? };
                    self.set_caret(ec, &range, Some(controller.caret()))?;
                }
            }
        } else if let Some(active) = composition.take() {
            // The word was erased (e.g. Backspace on its last letter).
            let range = unsafe { active.GetRange()? };
            unsafe { range.SetText(ec, 0, &[])? };
            unsafe { active.EndComposition(ec)? };
        }
        let anchor = match composition.as_ref() {
            Some(active) if controller.is_composing() => self.text_rect(ec, active),
            _ => None,
        };
        let candidate = self.shared.candidate.borrow();
        if let (Some(window), Some(anchor)) = (candidate.as_ref(), anchor) {
            let meanings = (controller.settings.meaning_language != 0
                && controller.language() == keyin::controller::Language::Chinese)
                .then(engine::meanings);
            window.show(&controller.panel(meanings), anchor);
        } else if let Some(window) = candidate.as_ref() {
            window.hide();
        }
        Ok(())
    }

    fn text_rect(&self, ec: u32, composition: &ITfComposition) -> Option<RECT> {
        let range = unsafe { composition.GetRange() }.ok()?;
        let view = unsafe { self.context.GetActiveView() }.ok()?;
        let mut rect = RECT::default();
        let mut clipped = BOOL::default();
        unsafe { view.GetTextExt(ec, &range, &mut rect, &mut clipped) }.ok()?;
        Some(rect)
    }
}

#[implement(
    ITfTextInputProcessorEx,
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfFnConfigure,
    ITfFunction
)]
pub struct TextService {
    shared: Shared,
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    thread_mgr_cookie: Cell<u32>,
    langbar: RefCell<Option<ITfLangBarItemButton>>,
}

impl TextService {
    pub fn create() -> TextService {
        TextService {
            shared: Rc::new(Inner {
                controller: RefCell::new(None),
                composition: RefCell::new(None),
                composition_sink: RefCell::new(None),
                candidate: RefCell::new(None),
                langbar_sink: RefCell::new(None),
                client_id: Cell::new(0),
                eaten: Cell::new(false),
                replay: Cell::new(false),
            }),
            thread_mgr: RefCell::new(None),
            thread_mgr_cookie: Cell::new(0),
            langbar: RefCell::new(None),
        }
    }

    /// Whether KeyIn wants this key: everything while composing, letters when idle.
    fn wants(&self, event: Option<KeyEvent>) -> bool {
        let Some(event) = event else { return false };
        let Ok(controller) = self.shared.controller.try_borrow() else {
            return false;
        };
        let Some(controller) = controller.as_ref() else {
            return false;
        };
        // Idle keys are claimed too when they type a character: letters start a word and
        // every other character is typed as on QWERTY (see `us_char`).
        controller.is_composing() || (!event.shortcut && matches!(event.key, Key::Char(_)))
    }

    fn edit(&self, context: &ITfContext, action: Action, sync: bool) -> Result<()> {
        let session: ITfEditSession = EditSession {
            shared: self.shared.clone(),
            context: context.clone(),
            action,
        }
        .into();
        let flags = if sync {
            TF_ES_SYNC | TF_ES_READWRITE
        } else {
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE
        };
        let result =
            unsafe { context.RequestEditSession(self.shared.client_id.get(), &session, flags)? };
        result.ok()
    }

    /// Picks up changes made in the settings window (another process) between words.
    fn reload_settings(&self) {
        let changed = match self.shared.controller.try_borrow_mut() {
            Ok(mut controller) => match controller.as_mut() {
                Some(controller) if !controller.is_composing() => {
                    let settings = settings::load();
                    let changed = settings != controller.settings;
                    controller.set_settings(settings);
                    changed
                }
                _ => false,
            },
            Err(_) => false,
        };
        if changed {
            self.shared.notify_langbar();
        }
    }

    /// Commits a pending composition when focus moves away from its document.
    fn commit_pending(&self) {
        let context = self
            .shared
            .composition
            .borrow()
            .as_ref()
            .and_then(|composition| unsafe { composition.GetRange() }.ok())
            .and_then(|range| unsafe { range.GetContext() }.ok());
        if let Some(context) = context {
            let _ = self.edit(&context, Action::Commit, false);
        } else if let Ok(mut controller) = self.shared.controller.try_borrow_mut() {
            if let Some(controller) = controller.as_mut() {
                controller.reset();
            }
        }
        self.shared.hide_candidates();
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, thread_mgr: Option<&ITfThreadMgr>, client_id: u32) -> Result<()> {
        self.ActivateEx(thread_mgr, client_id, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        self.commit_pending();
        if let Some(thread_mgr) = self.thread_mgr.borrow_mut().take() {
            unsafe {
                if let Ok(keystrokes) = thread_mgr.cast::<ITfKeystrokeMgr>() {
                    let _ = keystrokes.UnadviseKeyEventSink(self.shared.client_id.get());
                }
                if let Ok(source) = thread_mgr.cast::<ITfSource>() {
                    let _ = source.UnadviseSink(self.thread_mgr_cookie.get());
                }
                if let (Ok(items), Some(button)) = (
                    thread_mgr.cast::<ITfLangBarItemMgr>(),
                    self.langbar.borrow_mut().take(),
                ) {
                    if let Ok(item) = button.cast::<ITfLangBarItem>() {
                        let _ = items.RemoveItem(&item);
                    }
                }
            }
        }
        self.shared.langbar_sink.borrow_mut().take();
        self.shared.composition.borrow_mut().take();
        self.shared.composition_sink.borrow_mut().take();
        self.shared.candidate.borrow_mut().take();
        if let Some(controller) = self.shared.controller.borrow_mut().as_mut() {
            controller.deactivate();
        }
        self.shared.controller.borrow_mut().take();
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(
        &self,
        thread_mgr: Option<&ITfThreadMgr>,
        client_id: u32,
        _flags: u32,
    ) -> Result<()> {
        let thread_mgr = thread_mgr.ok_or(windows::core::Error::from(
            windows::Win32::Foundation::E_INVALIDARG,
        ))?;
        self.shared.client_id.set(client_id);
        let rime = if engine::rime_ready() {
            Session::new()
        } else {
            None
        };
        let mut controller = Controller::new(settings::load(), rime, Box::new(engine::simplify));
        controller.activate();
        *self.shared.controller.borrow_mut() = Some(controller);
        *self.shared.candidate.borrow_mut() = CandidateWindow::new();
        let composition_sink: ITfCompositionSink =
            ComObjectInterface::<ITfCompositionSink>::as_interface_ref(self).to_owned();
        *self.shared.composition_sink.borrow_mut() = Some(composition_sink);
        unsafe {
            let key_sink: ITfKeyEventSink =
                ComObjectInterface::<ITfKeyEventSink>::as_interface_ref(self).to_owned();
            thread_mgr
                .cast::<ITfKeystrokeMgr>()?
                .AdviseKeyEventSink(client_id, &key_sink, true)?;
            let events: ITfThreadMgrEventSink =
                ComObjectInterface::<ITfThreadMgrEventSink>::as_interface_ref(self).to_owned();
            let cookie = thread_mgr
                .cast::<ITfSource>()?
                .AdviseSink(&ITfThreadMgrEventSink::IID, &events)?;
            self.thread_mgr_cookie.set(cookie);
            let button = langbar::LangBarButton::create(self.shared.clone());
            if let Ok(items) = thread_mgr.cast::<ITfLangBarItemMgr>() {
                let _ = items.AddItem(&button.cast::<ITfLangBarItem>()?);
            }
            *self.langbar.borrow_mut() = Some(button);
        }
        *self.thread_mgr.borrow_mut() = Some(thread_mgr.clone());
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, foreground: BOOL) -> Result<()> {
        if !foreground.as_bool() {
            self.shared.hide_candidates();
        }
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(self.wants(key_event(wparam)).into())
    }

    fn OnKeyDown(
        &self,
        context: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let event = key_event(wparam);
        let (Some(context), Some(event)) = (context, event) else {
            return Ok(false.into());
        };
        self.reload_settings();
        if !self.wants(Some(event)) {
            return Ok(false.into());
        }
        self.shared.eaten.set(false);
        self.shared.replay.set(false);
        self.edit(context, Action::Key(event), true)?;
        if self.shared.replay.take() {
            // The word is committed and KeyIn is idle, so the replayed key reaches the app.
            replay(wparam);
        }
        Ok(self.shared.eaten.get().into())
    }

    fn OnTestKeyUp(&self, _: Option<&ITfContext>, _: WPARAM, _: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnKeyUp(&self, _: Option<&ITfContext>, _: WPARAM, _: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnPreservedKey(&self, _: Option<&ITfContext>, _: *const GUID) -> Result<BOOL> {
        Ok(false.into())
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(&self, _: u32, _: Option<&ITfComposition>) -> Result<()> {
        // The application ended the composition (e.g. a click); its text stays as typed.
        if let Ok(mut composition) = self.shared.composition.try_borrow_mut() {
            composition.take();
        }
        if let Ok(mut controller) = self.shared.controller.try_borrow_mut() {
            if let Some(controller) = controller.as_mut() {
                controller.reset();
            }
        }
        self.shared.hide_candidates();
        Ok(())
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnSetFocus(&self, _: Option<&ITfDocumentMgr>, _: Option<&ITfDocumentMgr>) -> Result<()> {
        self.commit_pending();
        Ok(())
    }

    fn OnPushContext(&self, _: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
}

/// "Options" for KeyIn in Windows language settings opens the settings window.
impl ITfFnConfigure_Impl for TextService_Impl {
    fn Show(&self, _: HWND, _: u16, _: *const GUID) -> Result<()> {
        settings_window::open();
        Ok(())
    }
}

impl ITfFunction_Impl for TextService_Impl {
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(BSTR::from("KeyIn"))
    }
}
