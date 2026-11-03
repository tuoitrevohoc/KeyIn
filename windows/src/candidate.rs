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
            EndPaint, GetDC, GetStockObject, GetTextExtentPoint32W, Rectangle, ReleaseDC,
            RoundRect, SelectObject, SetBkMode, SetTextColor, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE,
            DT_VCENTER, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, NULL_PEN, PAINTSTRUCT, PS_SOLID,
            TRANSPARENT,
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
    label: String,
    meaning: String,
    rect: RECT,
}

#[derive(Default)]
struct Layout {
    items: Vec<Item>,
    badge: String,
    hints: String,
    language: Option<Language>,
    width: i32,
    height: i32,
}

thread_local! {
    // TSF calls a text service on its thread only, so each thread draws its own window.
    static LAYOUT: RefCell<Layout> = RefCell::new(Layout::default());
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

/// Which font draws a character: Arial for Latin and Vietnamese (its diacritics stay
/// legible), Microsoft YaHei UI for Chinese, Segoe UI Symbol for anything else (⇧).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Script {
    Latin,
    Chinese,
    Symbol,
}

fn script(c: char) -> Script {
    match c as u32 {
        0..=0x024f | 0x0300..=0x036f | 0x1e00..=0x1eff | 0x2000..=0x206f | 0x2190..=0x2193 => {
            Script::Latin
        }
        0x2e80..=0x9fff | 0xf900..=0xfaff | 0xfe30..=0xfe4f | 0xff00..=0xffef | 0x20000.. => {
            Script::Chinese
        }
        _ => Script::Symbol,
    }
}

/// Splits text into runs that share a font.
fn runs(text: &str) -> Vec<(Script, Vec<u16>)> {
    let mut runs: Vec<(Script, Vec<u16>)> = Vec::new();
    for c in text.chars() {
        let kind = script(c);
        let mut units = [0u16; 2];
        let encoded = c.encode_utf16(&mut units);
        match runs.last_mut() {
            Some((last, run)) if *last == kind || c == ' ' => run.extend_from_slice(encoded),
            _ => runs.push((kind, encoded.to_vec())),
        }
    }
    runs
}

fn measure(dc: HDC, text: &[u16]) -> i32 {
    let mut size = SIZE::default();
    let _ = unsafe { GetTextExtentPoint32W(dc, text, &mut size) };
    size.cx
}

struct Fonts {
    latin: HFONT,
    chinese: HFONT,
    symbol: HFONT,
}

fn create_font(height: i32, weight: u32, face: PCWSTR) -> HFONT {
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
            face,
        )
    }
}

impl Fonts {
    fn new(height: i32, weight: u32) -> Fonts {
        Fonts {
            latin: create_font(height, weight, w!("Arial")),
            chinese: create_font(height, weight, w!("Microsoft YaHei UI")),
            symbol: create_font(height, weight, w!("Segoe UI Symbol")),
        }
    }

    fn select(&self, dc: HDC, kind: Script) {
        let font = match kind {
            Script::Latin => self.latin,
            Script::Chinese => self.chinese,
            Script::Symbol => self.symbol,
        };
        unsafe { SelectObject(dc, font) };
    }

    fn width(&self, dc: HDC, text: &str) -> i32 {
        runs(text)
            .iter()
            .map(|(kind, run)| {
                self.select(dc, *kind);
                measure(dc, run)
            })
            .sum()
    }

    /// Draws `text` left-aligned and vertically centred in `rect`, run by run.
    fn draw(&self, dc: HDC, text: &str, mut rect: RECT) {
        for (kind, mut run) in runs(text) {
            self.select(dc, kind);
            let width = measure(dc, &run);
            unsafe {
                DrawTextW(
                    dc,
                    &mut run,
                    &mut rect,
                    DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
                )
            };
            rect.left += width;
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.latin);
            let _ = DeleteObject(self.chinese);
            let _ = DeleteObject(self.symbol);
        }
    }
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
        let regular = Fonts::new(17, FW_NORMAL.0);
        let small = Fonts::new(13, FW_NORMAL.0);
        let bold = Fonts::new(13, FW_SEMIBOLD.0);
        let previous = unsafe { SelectObject(dc, regular.latin) };
        let mut layout = Layout {
            badge: panel.language.badge().to_owned(),
            hints: format!("   {}", panel.hints.join("   ")),
            language: Some(panel.language),
            ..Default::default()
        };
        let mut x = PADDING;
        let mut y = PADDING;
        let mut width = 0;
        for item in &panel.items {
            let label = format!("{} {}", item.number, item.text);
            let meaning = item
                .meaning
                .as_ref()
                .map(|m| format!("  {m}"))
                .unwrap_or_default();
            let item_width = regular.width(dc, &label) + small.width(dc, &meaning) + 2 * PADDING;
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
        let footer_width =
            PADDING + bold.width(dc, &layout.badge) + small.width(dc, &layout.hints) + PADDING;
        unsafe {
            SelectObject(dc, previous);
            ReleaseDC(self.hwnd, dc);
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
            let regular = Fonts::new(17, FW_NORMAL.0);
            let small = Fonts::new(13, FW_NORMAL.0);
            let bold = Fonts::new(13, FW_SEMIBOLD.0);
            let old_font = SelectObject(dc, regular.latin);
            for (index, item) in layout.items.iter().enumerate() {
                let mut rect = item.rect;
                if index == 0 {
                    SelectObject(dc, accent);
                    SelectObject(dc, GetStockObject(NULL_PEN));
                    let _ = RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, 12, 12);
                    SetTextColor(dc, rgb(0xff, 0xff, 0xff));
                } else {
                    SetTextColor(dc, rgb(0x20, 0x20, 0x20));
                }
                rect.left += PADDING;
                regular.draw(dc, &item.label, rect);
                if !item.meaning.is_empty() {
                    rect.left += regular.width(dc, &item.label);
                    SetTextColor(
                        dc,
                        if index == 0 {
                            rgb(0xe8, 0xf0, 0xfe)
                        } else {
                            rgb(0x70, 0x70, 0x70)
                        },
                    );
                    small.draw(dc, &item.meaning, rect);
                }
            }
            let mut footer = RECT {
                left: PADDING,
                top: layout.height - FOOTER_HEIGHT,
                right: layout.width - PADDING,
                bottom: layout.height,
            };
            SetTextColor(dc, badge_color(layout.language));
            bold.draw(dc, &layout.badge, footer);
            footer.left += bold.width(dc, &layout.badge);
            SetTextColor(dc, rgb(0x70, 0x70, 0x70));
            small.draw(dc, &layout.hints, footer);
            SelectObject(dc, old_font);
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
            let _ = DeleteObject(background);
            let _ = DeleteObject(border);
            let _ = DeleteObject(accent);
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
