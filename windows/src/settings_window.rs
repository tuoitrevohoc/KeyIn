//! The "KeyIn – Settings" window, a small form like the macOS settings panel. It runs in
//! its own process (`rundll32 keyin_tsf.dll,KeyInSettings`) so no application hosting the
//! text service ever shows modal UI; changes are saved immediately and every input
//! context picks them up on its next key press.
use crate::{engine, settings, tr};
use keyin::controller::Settings;
use std::ffi::c_void;
use windows::{
    core::{w, HSTRING, PCWSTR, PSTR},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{CreateFontW, GetSysColorBrush, COLOR_BTNFACE, FW_NORMAL, HFONT},
        System::ApplicationInstallationAndServicing::{ActivateActCtx, CreateActCtxW, ACTCTXW},
        UI::{
            Controls::BST_CHECKED,
            HiDpi::{
                GetDpiForSystem, SetThreadDpiAwarenessContext,
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            },
            Input::KeyboardAndMouse::EnableWindow,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW,
                GetDlgItem, GetMessageW, IsDialogMessageW, LoadIconW, PostQuitMessage,
                RegisterClassW, SendMessageW, SetForegroundWindow, ShowWindow, TranslateMessage,
                BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTOCHECKBOX, BS_PUSHBUTTON,
                CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
                CW_USEDEFAULT, HMENU, MSG, SW_SHOWNORMAL, WINDOW_EX_STYLE, WINDOW_STYLE,
                WM_COMMAND, WM_DESTROY, WM_SETFONT, WNDCLASSW, WS_CAPTION, WS_CHILD,
                WS_MINIMIZEBOX, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
            },
        },
    },
};

const CLASS: PCWSTR = w!("KeyInSettingsWindow");
const ID_LANGUAGE: i32 = 100; // + 1..=3: enabled languages
const ID_AUTO: i32 = 110;
const ID_MANUAL: i32 = 111;
const ID_AUTOCORRECT: i32 = 120;
const ID_TABS: i32 = 121;
const ID_LEARN: i32 = 122;
const ID_MEANINGS: i32 = 130;
const ID_CLOSE: i32 = 2; // IDCANCEL: Esc closes too.

/// Comctl32 v6 gives the controls the current Windows look inside rundll32.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly>
    <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
      processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
  </dependentAssembly></dependency>
</assembly>"#;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// Opens the settings window in a separate process.
pub fn open() {
    let dll = engine::install_dir().join("keyin_tsf.dll");
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
    let _ = std::process::Command::new(format!("{system}\\System32\\rundll32.exe"))
        .arg(format!("{},KeyInSettings", dll.display()))
        .spawn();
}

struct Form {
    hwnd: HWND,
    scale: f32,
    font: HFONT,
}

impl Form {
    fn px(&self, value: i32) -> i32 {
        (value as f32 * self.scale).round() as i32
    }

    fn control(
        &self,
        class: PCWSTR,
        text: &str,
        style: u32,
        id: i32,
        rect: (i32, i32, i32, i32),
    ) -> HWND {
        let text = wide(text);
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                self.px(rect.0),
                self.px(rect.1),
                self.px(rect.2),
                self.px(rect.3),
                self.hwnd,
                HMENU(id as isize as *mut c_void),
                crate::module(),
                None,
            )
        }
        .unwrap_or_default();
        unsafe { SendMessageW(hwnd, WM_SETFONT, WPARAM(self.font.0 as usize), LPARAM(1)) };
        hwnd
    }

    fn checkbox(&self, text: &str, id: i32, x: i32, y: i32, width: i32) {
        self.control(
            w!("BUTTON"),
            text,
            (BS_AUTOCHECKBOX as u32) | WS_TABSTOP.0,
            id,
            (x, y, width, 22),
        );
    }

    fn combo(&self, items: &[&str], id: i32, x: i32, y: i32, width: i32) {
        let combo = self.control(
            w!("COMBOBOX"),
            "",
            (CBS_DROPDOWNLIST as u32) | WS_TABSTOP.0 | WS_VSCROLL.0,
            id,
            (x, y, width, 200),
        );
        for item in items {
            let item = wide(item);
            unsafe {
                SendMessageW(
                    combo,
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(item.as_ptr() as isize),
                )
            };
        }
    }

    fn label(&self, text: &str, x: i32, y: i32, width: i32) {
        self.control(w!("STATIC"), text, 0, -1, (x, y + 3, width, 20));
    }
}

fn item(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(hwnd, id) }.unwrap_or_default()
}

fn checked(hwnd: HWND, id: i32) -> bool {
    unsafe { SendMessageW(item(hwnd, id), BM_GETCHECK, WPARAM(0), LPARAM(0)) }.0
        == BST_CHECKED.0 as isize
}

fn set_checked(hwnd: HWND, id: i32, value: bool) {
    unsafe {
        SendMessageW(
            item(hwnd, id),
            BM_SETCHECK,
            WPARAM(value as usize),
            LPARAM(0),
        )
    };
}

fn selection(hwnd: HWND, id: i32) -> u32 {
    unsafe { SendMessageW(item(hwnd, id), CB_GETCURSEL, WPARAM(0), LPARAM(0)) }
        .0
        .max(0) as u32
}

fn enable(hwnd: HWND, id: i32, enabled: bool) {
    let _ = unsafe { EnableWindow(item(hwnd, id), enabled) };
}

/// Shows `settings` in the form, including which controls apply.
fn sync(hwnd: HWND, settings: &Settings) {
    let enabled = settings.enabled();
    let single = enabled & (enabled - 1) == 0;
    for language in 1..=3 {
        set_checked(hwnd, ID_LANGUAGE + language, enabled & (1 << language) != 0);
    }
    set_checked(hwnd, ID_AUTO, settings.automatic && !single);
    enable(hwnd, ID_AUTO, !single);
    unsafe {
        SendMessageW(
            item(hwnd, ID_MANUAL),
            CB_SETCURSEL,
            WPARAM(settings.mode().max(1) as usize - 1),
            LPARAM(0),
        )
    };
    enable(hwnd, ID_MANUAL, !single && !settings.automatic);
    set_checked(hwnd, ID_AUTOCORRECT, settings.autocorrect);
    set_checked(hwnd, ID_TABS, settings.tab_shortcuts);
    set_checked(hwnd, ID_LEARN, settings.learn_chinese);
    enable(hwnd, ID_LEARN, enabled & 8 != 0);
    unsafe {
        SendMessageW(
            item(hwnd, ID_MEANINGS),
            CB_SETCURSEL,
            WPARAM(settings.meaning_language as usize),
            LPARAM(0),
        )
    };
    enable(hwnd, ID_MEANINGS, enabled & 8 != 0);
}

/// Reads the form after a change; `changed` is the control the user touched.
fn read(hwnd: HWND, changed: i32) -> Settings {
    let mut settings = settings::load();
    let mut enabled = (1..=3u32)
        .filter(|&l| checked(hwnd, ID_LANGUAGE + l as i32))
        .fold(0, |mask, l| mask | (1 << l));
    if enabled == 0 {
        // At least one language always stays enabled: undo unchecking the last one.
        enabled = 1 << (changed - ID_LANGUAGE).clamp(1, 3);
    }
    settings.enabled_languages = enabled;
    // Disabled controls show derived values, so only the touched control is authoritative.
    if changed == ID_AUTO {
        settings.automatic = checked(hwnd, ID_AUTO);
    }
    if changed == ID_MANUAL {
        settings.language = selection(hwnd, ID_MANUAL) + 1;
    }
    if enabled & (1 << settings.language) == 0 {
        settings.language = (1..=3).find(|l| enabled & (1 << l) != 0).unwrap_or(1);
    }
    settings.autocorrect = checked(hwnd, ID_AUTOCORRECT);
    settings.tab_shortcuts = checked(hwnd, ID_TABS);
    settings.learn_chinese = checked(hwnd, ID_LEARN);
    settings.meaning_language = selection(hwnd, ID_MEANINGS).min(2);
    settings
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            let code = ((wparam.0 >> 16) & 0xffff) as u32;
            if id == ID_CLOSE {
                let _ = unsafe { DestroyWindow(hwnd) };
            } else if (code == BN_CLICKED || code == CBN_SELCHANGE) && id > ID_LANGUAGE {
                let settings = read(hwnd, id);
                settings::save(&settings);
                sync(hwnd, &settings);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// rundll32 entry point: `rundll32.exe keyin_tsf.dll,KeyInSettings`.
#[no_mangle]
pub extern "system" fn KeyInSettings(_: HWND, _: HINSTANCE, _: PSTR, _: i32) {
    if let Ok(existing) = unsafe { FindWindowW(CLASS, PCWSTR::null()) } {
        let _ = unsafe { SetForegroundWindow(existing) };
        return;
    }
    unsafe {
        let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let manifest = std::env::temp_dir().join("keyin-settings.manifest");
        if std::fs::write(&manifest, MANIFEST).is_ok() {
            let path = wide(&manifest.to_string_lossy());
            let context = ACTCTXW {
                cbSize: std::mem::size_of::<ACTCTXW>() as u32,
                lpSource: PCWSTR(path.as_ptr()),
                ..Default::default()
            };
            if let Ok(handle) = CreateActCtxW(&context) {
                let mut cookie = 0;
                let _ = ActivateActCtx(handle, &mut cookie);
            }
        }
    }
    let instance = crate::module().into();
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        hIcon: unsafe { LoadIconW(crate::module(), crate::ICON_RESOURCE) }.unwrap_or_default(),
        lpszClassName: CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassW(&class) };
    let scale = unsafe { GetDpiForSystem() } as f32 / 96.0;
    let px = |value: i32| (value as f32 * scale).round() as i32;
    let Ok(hwnd) = (unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            &HSTRING::from(tr("KeyIn – Cài đặt", "KeyIn – Settings")),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            px(400),
            px(330),
            None,
            None,
            instance,
            None,
        )
    }) else {
        return;
    };
    // Segoe UI renders Vietnamese diacritics cleanly at UI sizes.
    let font = unsafe {
        CreateFontW(
            -px(15),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            1,
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            w!("Segoe UI"),
        )
    };
    let form = Form { hwnd, scale, font };
    form.label(tr("Ngôn ngữ:", "Languages:"), 16, 16, 100);
    for (index, name) in [
        (1, tr("Tiếng Anh", "English")),
        (2, tr("Tiếng Việt", "Vietnamese")),
        (3, tr("Tiếng Trung", "Chinese")),
    ] {
        form.checkbox(name, ID_LANGUAGE + index, 120 + (index - 1) * 88, 16, 88);
    }
    form.checkbox(
        tr(
            "Tự nhận diện ngôn ngữ (Auto)",
            "Detect language automatically (Auto)",
        ),
        ID_AUTO,
        16,
        48,
        340,
    );
    form.label(tr("Khi tắt Auto, gõ:", "Without Auto, type:"), 16, 78, 112);
    form.combo(
        &[
            tr("Tiếng Anh", "English"),
            tr("Tiếng Việt (Telex)", "Vietnamese (Telex)"),
            tr("Tiếng Trung (Pinyin)", "Chinese (Pinyin)"),
        ],
        ID_MANUAL,
        130,
        78,
        230,
    );
    form.checkbox(
        tr("Tự sửa lỗi gõ", "Fix typos"),
        ID_AUTOCORRECT,
        16,
        114,
        340,
    );
    form.checkbox(
        tr(
            "Tab đổi tiếng, Shift+Tab bật/tắt Auto khi đang gõ",
            "Tab switches language, Shift+Tab toggles Auto while typing",
        ),
        ID_TABS,
        16,
        140,
        360,
    );
    form.checkbox(
        tr(
            "Học lựa chọn tiếng Trung (lưu trên máy)",
            "Learn Chinese choices (saved on this PC)",
        ),
        ID_LEARN,
        16,
        166,
        360,
    );
    form.label(
        tr("Nghĩa từ tiếng Trung:", "Chinese meanings:"),
        16,
        200,
        130,
    );
    form.combo(
        &[tr("Tắt", "Off"), "Tiếng Việt", "English"],
        ID_MEANINGS,
        150,
        200,
        210,
    );
    form.control(
        w!("BUTTON"),
        tr("Đóng", "Close"),
        (BS_PUSHBUTTON as u32) | WS_TABSTOP.0,
        ID_CLOSE,
        (284, 244, 90, 28),
    );
    sync(hwnd, &settings::load());
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(hwnd);
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            // Tab / Esc navigation between controls, like a dialog.
            if !IsDialogMessageW(hwnd, &message).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
