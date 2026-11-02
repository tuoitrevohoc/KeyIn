//! Candidate window: a non-activating topmost popup drawn with GDI.
//! One row of candidates (first highlighted) above a footer with the language badge and
//! key hints; with Chinese meanings enabled, one candidate per line.
use keyin::controller::{Language, Panel};
use std::cell::RefCell;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW,
            EndPaint, GetDC, GetTextExtentPoint32W, Rectangle, ReleaseDC, RoundRect, SelectObject,
            SetBkMode, SetTextColor, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_NORMAL,
            FW_SEMIBOLD, HDC, HFONT, HGDIOBJ, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, RegisterClassW,
            SetWindowPos, ShowWindow, CS_DROPSHADOW, HWND_TOPMOST, SM_CXSCREEN, SM_CYSCREEN,
            SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

const PADDING: i32 = 8;
const ITEM_HEIGHT: i32 = 30;
const FOOTER_HEIGHT: i32 = 24;
const GAP: i32 = 4;

struct Item {
    label: Vec<u16>,
    meaning: Vec<u16>,
    rect: RECT,
}

#[derive(Default)]
struct Layout {
    items: Vec<Item>,
    badge: Vec<u16>,
    hints: Vec<u16>,
    language: Option<Language>,
    width: i32,
    height: i32,
}

thread_local! {
    // TSF calls a text service on its thread only, so each thread draws its own window.
    static LAYOUT: RefCell<Layout> = RefCell::new(Layout::default());
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

fn badge_color(language: Option<Language>) -> COLORREF {
    match language {
        Some(Language::Vietnamese) => rgb(0x00, 0x8b, 0x8b),
        Some(Language::Chinese) => rgb(0xe0, 0x6c, 0x00),
        _ => rgb(0x1a, 0x73, 0xe8),
    }
}

fn font(height: i32, weight: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            weight as i32,
            0,
            0,
            0,
            1,
            Default::default(),
            Default::default(),
            Default::default(),
            0,
            w!("Microsoft YaHei UI"),
        )
    }
}

fn measure(dc: HDC, text: &[u16]) -> i32 {
    let mut size = SIZE::default();
    let _ = unsafe { GetTextExtentPoint32W(dc, text, &mut size) };
    size.cx
}

pub struct CandidateWindow {
    hwnd: HWND,
}

impl CandidateWindow {
    pub fn new() -> Option<CandidateWindow> {
        let instance = crate::module().into();
        let class = WNDCLASSW {
            style: CS_DROPSHADOW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("KeyInCandidateWindow"),
            ..Default::default()
        };
        // Fails harmlessly when another text service instance already registered it.
        unsafe { RegisterClassW(&class) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                w!("KeyInCandidateWindow"),
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                instance,
                None,
            )
        }
        .ok()?;
        Some(CandidateWindow { hwnd })
    }

    /// `anchor` is the composition's screen rectangle; the window goes below it, or above
    /// when there is no room.
    pub fn show(&self, panel: &Panel, anchor: RECT) {
        let dc = unsafe { GetDC(self.hwnd) };
        let regular = font(17, FW_NORMAL.0);
        let small = font(13, FW_NORMAL.0);
        let previous = unsafe { SelectObject(dc, regular) };
        let mut layout = Layout {
            badge: wide(panel.language.badge()),
            hints: wide(&format!("   {}", panel.hints.join("   "))),
            language: Some(panel.language),
            ..Default::default()
        };
        let mut x = PADDING;
        let mut y = PADDING;
        let mut width = 0;
        for item in &panel.items {
            let label = wide(&format!("{} {}", item.number, item.text));
            let meaning = wide(
                &item
                    .meaning
                    .as_ref()
                    .map(|m| format!("  {m}"))
                    .unwrap_or_default(),
            );
            let label_width = measure(dc, &label);
            unsafe { SelectObject(dc, small) };
            let meaning_width = measure(dc, &meaning);
            unsafe { SelectObject(dc, regular) };
            let item_width = label_width + meaning_width + 2 * PADDING;
            let rect = if panel.show_meanings {
                let rect = RECT {
                    left: PADDING,
                    top: y,
                    right: PADDING + item_width,
                    bottom: y + ITEM_HEIGHT,
                };
                y += ITEM_HEIGHT;
                rect
            } else {
                let rect = RECT {
                    left: x,
                    top: y,
                    right: x + item_width,
                    bottom: y + ITEM_HEIGHT,
                };
                x += item_width + GAP;
                rect
            };
            width = width.max(rect.right);
            layout.items.push(Item {
                label,
                meaning,
                rect,
            });
        }
        if !panel.show_meanings && !layout.items.is_empty() {
            y += ITEM_HEIGHT;
        }
        unsafe { SelectObject(dc, small) };
        let footer_width =
            PADDING + measure(dc, &layout.badge) + measure(dc, &layout.hints) + PADDING;
        unsafe {
            SelectObject(dc, previous);
            ReleaseDC(self.hwnd, dc);
            let _ = DeleteObject(regular);
            let _ = DeleteObject(small);
        }
        layout.width = (width + PADDING).max(footer_width);
        layout.height = y + GAP + FOOTER_HEIGHT;
        let (screen_width, screen_height) =
            unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        let left = anchor.left.min(screen_width - layout.width).max(0);
        let mut top = anchor.bottom + 4;
        if top + layout.height > screen_height {
            top = (anchor.top - layout.height - 4).max(0);
        }
        let (w, h) = (layout.width, layout.height);
        LAYOUT.with(|cell| *cell.borrow_mut() = layout);
        unsafe {
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, left, top, w, h, SWP_NOACTIVATE);
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(self.hwnd, None, true);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

impl Drop for CandidateWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn paint(dc: HDC) {
    LAYOUT.with(|cell| {
        let layout = cell.borrow();
        unsafe {
            let background = CreateSolidBrush(rgb(0xfb, 0xfb, 0xfb));
            let border = CreatePen(PS_SOLID, 1, rgb(0xd0, 0xd0, 0xd0));
            let accent = CreateSolidBrush(rgb(0x1a, 0x73, 0xe8));
            let old_pen = SelectObject(dc, border);
            let old_brush = SelectObject(dc, background);
            let _ = Rectangle(dc, 0, 0, layout.width, layout.height);
            SetBkMode(dc, TRANSPARENT);
            let regular = font(17, FW_NORMAL.0);
            let small = font(13, FW_NORMAL.0);
            let bold = font(13, FW_SEMIBOLD.0);
            let old_font = SelectObject(dc, regular);
            for (index, item) in layout.items.iter().enumerate() {
                let mut rect = item.rect;
                if index == 0 {
                    SelectObject(dc, accent);
                    SelectObject(
                        dc,
                        HGDIOBJ(
                            windows::Win32::Graphics::Gdi::GetStockObject(
                                windows::Win32::Graphics::Gdi::NULL_PEN,
                            )
                            .0,
                        ),
                    );
                    let _ = RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, 12, 12);
                    SetTextColor(dc, rgb(0xff, 0xff, 0xff));
                } else {
                    SetTextColor(dc, rgb(0x20, 0x20, 0x20));
                }
                rect.left += PADDING;
                let mut label = item.label.clone();
                DrawTextW(
                    dc,
                    &mut label,
                    &mut rect,
                    DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
                );
                if !item.meaning.is_empty() {
                    rect.left += measure(dc, &item.label);
                    SelectObject(dc, small);
                    SetTextColor(
                        dc,
                        if index == 0 {
                            rgb(0xe8, 0xf0, 0xfe)
                        } else {
                            rgb(0x70, 0x70, 0x70)
                        },
                    );
                    let mut meaning = item.meaning.clone();
                    DrawTextW(
                        dc,
                        &mut meaning,
                        &mut rect,
                        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
                    );
                    SelectObject(dc, regular);
                }
            }
            let top = layout.height - FOOTER_HEIGHT;
            let mut footer = RECT {
                left: PADDING,
                top,
                right: layout.width - PADDING,
                bottom: layout.height,
            };
            SelectObject(dc, bold);
            SetTextColor(dc, badge_color(layout.language));
            let mut badge = layout.badge.clone();
            DrawTextW(
                dc,
                &mut badge,
                &mut footer,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
            footer.left += measure(dc, &layout.badge);
            SelectObject(dc, small);
            SetTextColor(dc, rgb(0x70, 0x70, 0x70));
            let mut hints = layout.hints.clone();
            DrawTextW(
                dc,
                &mut hints,
                &mut footer,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
            SelectObject(dc, old_font);
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
            for object in [
                regular.into(),
                small.into(),
                bold.into(),
                background.into(),
                border.into(),
                accent.into(),
            ] {
                let _: HGDIOBJ = object;
                let _ = DeleteObject(object);
            }
        }
    });
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = unsafe { BeginPaint(hwnd, &mut paint_struct) };
        paint(dc);
        unsafe {
            let _ = EndPaint(hwnd, &paint_struct);
        }
        return LRESULT(0);
    }
    // Clicks never take focus from the document.
    if message == windows::Win32::UI::WindowsAndMessaging::WM_MOUSEACTIVATE {
        return LRESULT(windows::Win32::UI::WindowsAndMessaging::MA_NOACTIVATE as isize);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
