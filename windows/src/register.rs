//! regsvr32 / MSI self-registration: COM class, TSF profile and categories.
use crate::{engine, CLSID_KEYIN, PROFILE_KEYIN};
use windows::{
    core::{w, Result, HSTRING, PCWSTR},
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            },
            Registry::{RegDeleteTreeW, RegSetKeyValueW, HKEY_LOCAL_MACHINE, REG_SZ},
        },
        UI::{
            Input::KeyboardAndMouse::HKL,
            TextServices::{
                CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr,
                ITfInputProcessorProfileMgr, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIP_KEYBOARD,
            },
        },
    },
};

/// Vietnamese (Vietnam), matching the macOS input source's primary language.
const LANGID_VI_VN: u16 = 0x042a;
const CATEGORIES: [windows::core::GUID; 3] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
];

fn class_key() -> HSTRING {
    HSTRING::from(format!("Software\\Classes\\CLSID\\{{{:?}}}", CLSID_KEYIN))
}

fn set_string(key: &HSTRING, name: PCWSTR, value: &str) -> Result<()> {
    let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
    unsafe {
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            key,
            name,
            REG_SZ.0,
            Some(wide.as_ptr() as *const _),
            (wide.len() * 2) as u32,
        )
    }
    .ok()
}

pub fn register() -> Result<()> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dll = engine::install_dir().join("keyin_tsf.dll");
    let dll = dll.to_string_lossy();
    let key = class_key();
    set_string(&key, PCWSTR::null(), "KeyIn")?;
    let server = HSTRING::from(format!("{key}\\InprocServer32"));
    set_string(&server, PCWSTR::null(), &dll)?;
    set_string(&server, w!("ThreadingModel"), "Apartment")?;

    let profiles: ITfInputProcessorProfileMgr =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)? };
    let description: Vec<u16> = "KeyIn".encode_utf16().collect();
    let icon: Vec<u16> = dll.encode_utf16().collect();
    unsafe {
        profiles.RegisterProfile(
            &CLSID_KEYIN,
            LANGID_VI_VN,
            &PROFILE_KEYIN,
            &description,
            &icon,
            0,
            HKL::default(),
            0,
            true,
            0,
        )?
    };
    let categories: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
    for category in CATEGORIES {
        unsafe { categories.RegisterCategory(&CLSID_KEYIN, &category, &CLSID_KEYIN)? };
    }
    Ok(())
}

pub fn unregister() -> Result<()> {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if let Ok(categories) = unsafe {
        CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
    } {
        for category in CATEGORIES {
            let _ = unsafe { categories.UnregisterCategory(&CLSID_KEYIN, &category, &CLSID_KEYIN) };
        }
    }
    if let Ok(profiles) = unsafe {
        CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None,
            CLSCTX_INPROC_SERVER,
        )
    } {
        let _ =
            unsafe { profiles.UnregisterProfile(&CLSID_KEYIN, LANGID_VI_VN, &PROFILE_KEYIN, 0) };
    }
    let status: WIN32_ERROR = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, &class_key()) };
    if status.is_ok() || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        status.ok()
    }
}
