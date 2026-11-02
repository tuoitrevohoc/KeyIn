//! Per-user preferences in HKEY_CURRENT_USER\Software\KeyIn (DWORD values).
use keyin::controller::Settings;
use windows::{
    core::{w, PCWSTR},
    Win32::System::Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD,
    },
};

const KEY: PCWSTR = w!("Software\\KeyIn");

fn read(name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    }
    .is_ok()
    .then_some(value)
}

fn write(name: PCWSTR, value: u32) {
    let _ = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            REG_DWORD.0,
            Some(&value as *const u32 as *const _),
            std::mem::size_of::<u32>() as u32,
        )
    };
}

pub fn load() -> Settings {
    let defaults = Settings {
        // Every application process hosts its own Rime instance, and the learning
        // user dictionary can only be opened by one of them, so learning is opt-in.
        learn_chinese: false,
        ..Settings::default()
    };
    let flag = |name, default: bool| read(name).map_or(default, |value| value != 0);
    Settings {
        automatic: flag(w!("AutomaticDetection"), defaults.automatic),
        language: read(w!("Language"))
            .filter(|l| (1..=3).contains(l))
            .unwrap_or(defaults.language),
        autocorrect: flag(w!("Autocorrect"), defaults.autocorrect),
        learn_chinese: flag(w!("LearnChinese"), defaults.learn_chinese),
        meaning_language: read(w!("ChineseMeaningLanguage"))
            .filter(|m| *m <= 2)
            .unwrap_or(defaults.meaning_language),
        enabled_languages: read(w!("EnabledLanguages")).unwrap_or(defaults.enabled_languages),
        tab_shortcuts: flag(w!("TabShortcuts"), defaults.tab_shortcuts),
    }
}

pub fn save(settings: &Settings) {
    write(w!("AutomaticDetection"), settings.automatic as u32);
    write(w!("Language"), settings.language);
    write(w!("Autocorrect"), settings.autocorrect as u32);
    write(w!("LearnChinese"), settings.learn_chinese as u32);
    write(w!("ChineseMeaningLanguage"), settings.meaning_language);
    write(w!("EnabledLanguages"), settings.enabled_languages);
    write(w!("TabShortcuts"), settings.tab_shortcuts as u32);
}
