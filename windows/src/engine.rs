//! Process-wide resources next to the DLL: librime, Rime data and Chinese meanings.
use keyin::{meanings::Meanings, rime};
use std::{path::PathBuf, sync::OnceLock};
use windows::{
    core::w,
    Win32::{
        Foundation::LPARAM,
        Globalization::{LCMapStringEx, LCMAP_SIMPLIFIED_CHINESE},
        System::LibraryLoader::GetModuleFileNameW,
        UI::Shell::{FOLDERID_RoamingAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    },
};

/// Installation directory (the folder holding keyin_tsf.dll).
pub fn install_dir() -> PathBuf {
    let mut buffer = vec![0u16; 1024];
    let length = unsafe { GetModuleFileNameW(crate::module(), &mut buffer) } as usize;
    let path = PathBuf::from(String::from_utf16_lossy(&buffer[..length]));
    path.parent().map(PathBuf::from).unwrap_or_default()
}

fn user_dir() -> PathBuf {
    let base = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None) }
        .ok()
        .and_then(|path| unsafe { path.to_string() }.ok())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("KeyIn").join("Rime")
}

/// Starts librime once per process; false keeps the built-in Pinyin fallback.
pub fn rime_ready() -> bool {
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        let dir = install_dir();
        unsafe {
            let api = rime::load_library(&dir.join("rime.dll"));
            !api.is_null() && rime::start(api, &dir.join("rime"), &user_dir())
        }
    })
}

/// Loaded on first use only: the dictionary is large and most sessions never show meanings.
pub fn meanings() -> &'static Meanings {
    Meanings::load_shared(&install_dir().join("chinese-meanings.json"))
}

/// Traditional → Simplified for the built-in Pinyin fallback candidates.
pub fn simplify(text: &str) -> String {
    let source: Vec<u16> = text.encode_utf16().collect();
    if source.is_empty() {
        return String::new();
    }
    let mut target = vec![0u16; source.len() * 2 + 8];
    let length = unsafe {
        LCMapStringEx(
            w!("zh-CN"),
            LCMAP_SIMPLIFIED_CHINESE,
            &source,
            Some(&mut target),
            None,
            None,
            LPARAM(0),
        )
    };
    if length <= 0 {
        return text.to_owned();
    }
    String::from_utf16_lossy(&target[..length as usize])
}
