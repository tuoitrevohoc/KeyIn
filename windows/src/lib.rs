//! KeyIn for Windows: an in-process Text Services Framework (TSF) text input processor.
//! The engine, Rime binding and key logic live in the shared `keyin` crate; this crate
//! only adapts them to TSF, draws the candidate window and stores settings.
#![cfg(windows)]
#![allow(non_snake_case)]

mod candidate;
mod engine;
mod langbar;
mod register;
mod service;
mod settings;
mod settings_window;

use std::ffi::c_void;
use windows::{
    core::{implement, IUnknown, Interface, GUID, HRESULT},
    Win32::{
        Foundation::{
            BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_POINTER, HMODULE, S_FALSE,
        },
        System::{
            Com::{IClassFactory, IClassFactory_Impl},
            SystemServices::DLL_PROCESS_ATTACH,
        },
    },
};

/// COM class of the text service.
pub const CLSID_KEYIN: GUID = GUID::from_u128(0xd86a7d9d_eb03_4c09_a19b_b8d723c5939d);
/// Input profile registered under Vietnamese (vi-VN).
pub const PROFILE_KEYIN: GUID = GUID::from_u128(0x32967ad9_29d0_4724_99cd_26c0a0a907b8);

static mut MODULE: HMODULE = HMODULE(std::ptr::null_mut());

pub(crate) fn module() -> HMODULE {
    // SAFETY: written once in DllMain before any other entry point runs.
    unsafe { MODULE }
}

#[no_mangle]
extern "system" fn DllMain(module: HMODULE, reason: u32, _: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        unsafe { MODULE = module };
    }
    true.into()
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Option<&IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if object.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let service: IUnknown = service::TextService::create().into();
        unsafe { service.query(iid, object).ok() }
    }

    fn LockServer(&self, _: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

#[no_mangle]
unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || object.is_null() {
        return E_POINTER;
    }
    unsafe { *object = std::ptr::null_mut() };
    if unsafe { *clsid } != CLSID_KEYIN {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(iid, object) }
}

#[no_mangle]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    // Keep the process-wide Rime instance alive for the lifetime of the host process.
    S_FALSE
}

#[no_mangle]
extern "system" fn DllRegisterServer() -> HRESULT {
    register::register().map_or_else(|error| error.code(), |()| HRESULT(0))
}

#[no_mangle]
extern "system" fn DllUnregisterServer() -> HRESULT {
    register::unregister().map_or_else(|error| error.code(), |()| HRESULT(0))
}
