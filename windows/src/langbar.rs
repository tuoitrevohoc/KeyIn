//! Taskbar input-mode button: shows the active language and holds the settings menu.
//! Left click cycles Auto → EN → VI → 中; the menu mirrors the macOS settings window.
use crate::{service::Shared, CLSID_KEYIN};
use keyin::controller::Settings;
use windows::{
    core::{implement, IUnknown, Interface, Result, BSTR, GUID},
    Win32::{
        Foundation::{BOOL, E_INVALIDARG, POINT, RECT},
        Graphics::Gdi::{
            CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush,
            DeleteDC, DeleteObject, DrawTextW, FillRect, SelectObject, SetBkMode, SetTextColor,
            BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX,
            DT_SINGLELINE, DT_VCENTER, FW_BOLD, TRANSPARENT,
        },
        System::Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION},
        UI::{
            TextServices::{
                ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
                ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu, ITfSource, ITfSource_Impl,
                TfLBIClick, GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO, TF_LBI_CLK_LEFT,
                TF_LBI_STYLE_BTN_BUTTON, TF_LBI_STYLE_BTN_MENU, TF_LBI_STYLE_SHOWNINTRAY,
                TF_LBMENUF_CHECKED,
            },
            WindowsAndMessaging::{
                CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
            },
        },
    },
};

const SINK_COOKIE: u32 = 1;

#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct LangBarButton {
    shared: Shared,
}

impl LangBarButton {
    pub fn create(shared: Shared) -> ITfLangBarItemButton {
        LangBarButton { shared }.into()
    }
}

/// Label for the current mode: the pinned language, or "A" for automatic detection.
pub fn mode_text(settings: &Settings) -> &'static str {
    match settings.mode() {
        1 => "EN",
        2 => "VI",
        3 => "中",
        _ => "A",
    }
}

// Menu identifiers.
const MENU_AUTO: u32 = 1;
const MENU_LANGUAGE: u32 = 10; // + 1..=3
const MENU_ENABLED: u32 = 20; // + 1..=3
const MENU_AUTOCORRECT: u32 = 30;
const MENU_LEARN: u32 = 31;
const MENU_TABS: u32 = 32;
const MENU_MEANING: u32 = 40; // + 0..=2

fn menu_item(menu: &ITfMenu, id: u32, checked: bool, text: &str) {
    let text: Vec<u16> = text.encode_utf16().collect();
    let flags = if checked { TF_LBMENUF_CHECKED } else { 0 };
    let _ = unsafe { menu.AddMenuItem(id, flags, None, None, &text, std::ptr::null_mut()) };
}

impl ITfLangBarItem_Impl for LangBarButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if info.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let mut description = [0u16; 32];
        for (slot, unit) in description.iter_mut().zip("KeyIn".encode_utf16()) {
            *slot = unit;
        }
        unsafe {
            *info = TF_LANGBARITEMINFO {
                clsidService: CLSID_KEYIN,
                // The input-mode GUID puts the button in the Windows taskbar next to the clock.
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY,
                ulSort: 0,
                szDescription: description,
            }
        };
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    fn Show(&self, _: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(format!(
            "KeyIn – {}",
            mode_text(&self.shared.settings())
        )))
    }
}

impl ITfLangBarItemButton_Impl for LangBarButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _: &POINT, _: *const RECT) -> Result<()> {
        if click == TF_LBI_CLK_LEFT {
            self.shared.update_settings(|settings| {
                let enabled = settings.enabled();
                // Auto → each enabled language → Auto.
                let mut next = if settings.mode() == 0 {
                    0
                } else {
                    settings.language
                };
                loop {
                    next += 1;
                    if next > 3 {
                        settings.automatic = true;
                        break;
                    }
                    if enabled & (1 << next) != 0 {
                        settings.automatic = false;
                        settings.language = next;
                        break;
                    }
                }
            });
        }
        Ok(())
    }

    fn InitMenu(&self, menu: Option<&ITfMenu>) -> Result<()> {
        let Some(menu) = menu else { return Ok(()) };
        let settings = self.shared.settings();
        let mode = settings.mode();
        let enabled = settings.enabled();
        menu_item(menu, MENU_AUTO, mode == 0, "Tự nhận diện ngôn ngữ (Auto)");
        for (index, name) in [
            (1, "Tiếng Anh"),
            (2, "Tiếng Việt (Telex)"),
            (3, "Tiếng Trung (Pinyin giản thể)"),
        ] {
            menu_item(
                menu,
                MENU_LANGUAGE + index,
                mode == index,
                &format!("Gõ {name}"),
            );
        }
        for (index, name) in [(1, "EN"), (2, "VI"), (3, "中")] {
            menu_item(
                menu,
                MENU_ENABLED + index,
                enabled & (1 << index) != 0,
                &format!("Bật {name}"),
            );
        }
        menu_item(
            menu,
            MENU_AUTOCORRECT,
            settings.autocorrect,
            "Tự sửa lỗi gõ",
        );
        menu_item(
            menu,
            MENU_TABS,
            settings.tab_shortcuts,
            "Tab đổi tiếng, ⇧Tab bật/tắt Auto",
        );
        menu_item(
            menu,
            MENU_LEARN,
            settings.learn_chinese,
            "Học lựa chọn tiếng Trung (cục bộ)",
        );
        for (index, name) in [
            "Nghĩa tiếng Trung: Tắt",
            "Nghĩa tiếng Trung: Tiếng Việt",
            "Nghĩa tiếng Trung: English",
        ]
        .iter()
        .enumerate()
        {
            menu_item(
                menu,
                MENU_MEANING + index as u32,
                settings.meaning_language == index as u32,
                name,
            );
        }
        Ok(())
    }

    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        self.shared.update_settings(|settings| match id {
            MENU_AUTO => settings.automatic = !settings.automatic,
            id if (MENU_LANGUAGE + 1..=MENU_LANGUAGE + 3).contains(&id) => {
                settings.language = id - MENU_LANGUAGE;
                settings.automatic = false;
                settings.enabled_languages |= 1 << settings.language;
            }
            id if (MENU_ENABLED + 1..=MENU_ENABLED + 3).contains(&id) => {
                let mask = settings.enabled() ^ (1 << (id - MENU_ENABLED));
                // At least one language always stays enabled.
                if mask & 14 != 0 {
                    settings.enabled_languages = mask;
                }
            }
            MENU_AUTOCORRECT => settings.autocorrect = !settings.autocorrect,
            MENU_LEARN => settings.learn_chinese = !settings.learn_chinese,
            MENU_TABS => settings.tab_shortcuts = !settings.tab_shortcuts,
            id if (MENU_MEANING..=MENU_MEANING + 2).contains(&id) => {
                settings.meaning_language = id - MENU_MEANING
            }
            _ => {}
        });
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        icon(mode_text(&self.shared.settings()))
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(mode_text(&self.shared.settings())))
    }
}

impl ITfSource_Impl for LangBarButton_Impl {
    fn AdviseSink(&self, iid: *const GUID, sink: Option<&IUnknown>) -> Result<u32> {
        if unsafe { *iid } != ITfLangBarItemSink::IID {
            return Err(CONNECT_E_CANNOTCONNECT.into());
        }
        if self.shared.langbar_sink.borrow().is_some() {
            return Err(CONNECT_E_ADVISELIMIT.into());
        }
        let sink = sink
            .ok_or(windows::core::Error::from(E_INVALIDARG))?
            .cast::<ITfLangBarItemSink>()?;
        *self.shared.langbar_sink.borrow_mut() = Some(sink);
        Ok(SINK_COOKIE)
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != SINK_COOKIE || self.shared.langbar_sink.borrow_mut().take().is_none() {
            return Err(CONNECT_E_NOCONNECTION.into());
        }
        Ok(())
    }
}

/// Draws the mode label into a small-icon sized bitmap. The caller destroys the icon.
fn icon(text: &str) -> Result<HICON> {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    unsafe {
        let dc = CreateCompatibleDC(None);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        let mask = CreateBitmap(size, size, 1, 1, None);
        let old = SelectObject(dc, color);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: size,
            bottom: size,
        };
        let background = CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00e8_731a));
        FillRect(dc, &rect, background);
        let font = CreateFontW(
            -(size * 3 / 4),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            1,
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            windows::core::w!("Microsoft YaHei UI"),
        );
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00ff_ffff));
        let mut label: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(
            dc,
            &mut label,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        // GDI leaves alpha at zero; make every pixel opaque.
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
        for pixel in pixels {
            *pixel |= 0xff00_0000;
        }
        SelectObject(dc, old_font);
        SelectObject(dc, old);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(font);
        let _ = DeleteObject(background);
        let _ = DeleteObject(color);
        let _ = DeleteObject(mask);
        let _ = DeleteDC(dc);
        icon
    }
}
