//! Minimal librime binding through the versioned `rime_get_api` function table.
//! The host supplies the table, so the engine itself never links librime.
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::Path,
    ptr,
    sync::{
        atomic::{AtomicPtr, Ordering},
        OnceLock,
    },
};

type Bool = c_int;
type SessionId = usize;
type Unused = usize;

#[repr(C)]
struct Traits {
    data_size: c_int,
    shared_data_dir: *const c_char,
    user_data_dir: *const c_char,
    distribution_name: *const c_char,
    distribution_code_name: *const c_char,
    distribution_version: *const c_char,
    app_name: *const c_char,
    modules: *const *const c_char,
    min_log_level: c_int,
    log_dir: *const c_char,
    prebuilt_data_dir: *const c_char,
    staging_dir: *const c_char,
}

#[repr(C)]
struct Composition {
    length: c_int,
    cursor_pos: c_int,
    sel_start: c_int,
    sel_end: c_int,
    preedit: *mut c_char,
}

#[repr(C)]
struct Candidate {
    text: *mut c_char,
    comment: *mut c_char,
    reserved: *mut c_void,
}

#[repr(C)]
struct Menu {
    page_size: c_int,
    page_no: c_int,
    is_last_page: Bool,
    highlighted_candidate_index: c_int,
    num_candidates: c_int,
    candidates: *mut Candidate,
    select_keys: *mut c_char,
}

#[repr(C)]
struct Commit {
    data_size: c_int,
    text: *mut c_char,
}

#[repr(C)]
struct Context {
    data_size: c_int,
    composition: Composition,
    menu: Menu,
    commit_text_preview: *mut c_char,
    select_labels: *mut *mut c_char,
}

#[repr(C)]
struct CandidateListIterator {
    ptr: *mut c_void,
    index: c_int,
    candidate: Candidate,
}

/// Mirrors `RimeApi` from rime_api.h (librime 1.x) up to the last entry KeyIn uses.
#[repr(C)]
pub struct RimeApi {
    data_size: c_int,
    setup: unsafe extern "C" fn(*mut Traits),
    set_notification_handler: Unused,
    initialize: unsafe extern "C" fn(*mut Traits),
    finalize_to_sync_user_data: [Unused; 10],
    create_session: unsafe extern "C" fn() -> SessionId,
    find_session: Unused,
    destroy_session: unsafe extern "C" fn(SessionId) -> Bool,
    cleanup_sessions: [Unused; 2],
    process_key: unsafe extern "C" fn(SessionId, c_int, c_int) -> Bool,
    commit_composition: Unused,
    clear_composition: unsafe extern "C" fn(SessionId),
    get_commit: unsafe extern "C" fn(SessionId, *mut Commit) -> Bool,
    free_commit: unsafe extern "C" fn(*mut Commit) -> Bool,
    get_context: unsafe extern "C" fn(SessionId, *mut Context) -> Bool,
    free_context: unsafe extern "C" fn(*mut Context) -> Bool,
    status: [Unused; 2],
    set_option: unsafe extern "C" fn(SessionId, *const c_char, Bool),
    get_option_to_get_current_schema: [Unused; 6],
    select_schema: unsafe extern "C" fn(SessionId, *const c_char) -> Bool,
    schema_open_to_config_begin_list: [Unused; 34],
    get_input: unsafe extern "C" fn(SessionId) -> *const c_char,
    get_caret_pos: unsafe extern "C" fn(SessionId) -> usize,
    select_candidate: unsafe extern "C" fn(SessionId, usize) -> Bool,
    get_version: unsafe extern "C" fn() -> *const c_char,
    caret_and_page_selection: [Unused; 2],
    candidate_list_begin: unsafe extern "C" fn(SessionId, *mut CandidateListIterator) -> Bool,
    candidate_list_next: unsafe extern "C" fn(*mut CandidateListIterator) -> Bool,
    candidate_list_end: unsafe extern "C" fn(*mut CandidateListIterator),
}

static API: AtomicPtr<RimeApi> = AtomicPtr::new(ptr::null_mut());

fn api() -> Option<&'static RimeApi> {
    // SAFETY: only set by `start` from a table that lives as long as librime.
    unsafe { API.load(Ordering::Acquire).as_ref() }
}

fn struct_size<T>() -> c_int {
    (std::mem::size_of::<T>() - std::mem::size_of::<c_int>()) as c_int
}

fn text(raw: *const c_char) -> String {
    if raw.is_null() {
        String::new()
    } else {
        // SAFETY: librime returns NUL-terminated UTF-8 strings.
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Starts librime once per process with prebuilt shared data and a private user directory.
///
/// # Safety
/// `table` must be the pointer returned by librime's `rime_get_api`.
pub unsafe fn start(table: *const RimeApi, shared: &Path, user: &Path) -> bool {
    if api().is_some() {
        return true;
    }
    let Some(table) = (unsafe { table.as_ref() }) else {
        return false;
    };
    // Reject librime builds whose table ends before the entries used here.
    let required = std::mem::offset_of!(RimeApi, candidate_list_end) + std::mem::size_of::<usize>();
    if (table.data_size as usize + std::mem::size_of::<c_int>()) < required
        || !shared.join("build/keyin_pinyin.schema.yaml").exists()
        || std::fs::create_dir_all(user).is_err()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(user, std::fs::Permissions::from_mode(0o700));
    }
    static STRINGS: OnceLock<[CString; 2]> = OnceLock::new();
    let [shared, user] = STRINGS.get_or_init(|| {
        [shared, user]
            .map(|path| CString::new(path.to_string_lossy().as_bytes()).unwrap_or_default())
    });
    struct Modules([*const c_char; 2]);
    // SAFETY: the array only holds pointers to static string literals.
    unsafe impl Sync for Modules {}
    static MODULES: Modules = Modules([c"default".as_ptr(), ptr::null()]);
    let mut traits = Traits {
        data_size: struct_size::<Traits>(),
        shared_data_dir: shared.as_ptr(),
        user_data_dir: user.as_ptr(),
        distribution_name: c"KeyIn".as_ptr(),
        distribution_code_name: c"keyin".as_ptr(),
        distribution_version: c"0.3.0".as_ptr(),
        app_name: c"rime.keyin".as_ptr(),
        modules: MODULES.0.as_ptr(),
        min_log_level: 2,
        log_dir: c"".as_ptr(),
        prebuilt_data_dir: ptr::null(),
        staging_dir: ptr::null(),
    };
    unsafe {
        (table.setup)(&mut traits);
        (table.initialize)(&mut traits);
    }
    API.store(table as *const RimeApi as *mut RimeApi, Ordering::Release);
    true
}

pub fn version() -> Option<String> {
    api().map(|api| text(unsafe { (api.get_version)() }))
}

/// Opens librime dynamically and returns its API table (tests and the Windows host).
///
/// # Safety
/// Loading a library runs its initializers; `path` must name a trusted librime build.
pub unsafe fn load_library(path: &Path) -> *const RimeApi {
    type GetApi = unsafe extern "C" fn() -> *const RimeApi;
    #[cfg(unix)]
    let symbol = {
        extern "C" {
            fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        }
        let Ok(path) = CString::new(path.to_string_lossy().as_bytes()) else {
            return ptr::null();
        };
        let handle = unsafe { dlopen(path.as_ptr(), 2) };
        if handle.is_null() {
            return ptr::null();
        }
        unsafe { dlsym(handle, c"rime_get_api".as_ptr()) }
    };
    #[cfg(windows)]
    let symbol = {
        use std::os::windows::ffi::OsStrExt;
        extern "system" {
            fn LoadLibraryW(path: *const u16) -> *mut c_void;
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            return ptr::null();
        }
        unsafe { GetProcAddress(module, c"rime_get_api".as_ptr()) }
    };
    if symbol.is_null() {
        return ptr::null();
    }
    unsafe { std::mem::transmute::<*mut c_void, GetApi>(symbol)() }
}

pub struct Snapshot {
    pub preedit: String,
    /// Caret in UTF-16 code units, which both host text APIs use.
    pub caret: usize,
    pub choices: Vec<String>,
}

/// One isolated Rime session per input context.
pub struct Session {
    id: SessionId,
    learning: bool,
}

impl Session {
    pub fn new() -> Option<Session> {
        let api = api()?;
        let id = unsafe { (api.create_session)() };
        if id == 0 {
            return None;
        }
        if unsafe { (api.select_schema)(id, c"keyin_pinyin".as_ptr()) } == 0 {
            unsafe { (api.destroy_session)(id) };
            return None;
        }
        unsafe { (api.set_option)(id, c"ascii_mode".as_ptr(), 0) };
        Some(Session { id, learning: true })
    }

    fn api(&self) -> &'static RimeApi {
        api().expect("sessions exist only after start")
    }

    pub fn learning(&self) -> bool {
        self.learning
    }

    pub fn set_learning(&mut self, learning: bool) {
        if self.learning == learning {
            return;
        }
        self.clear();
        let schema = if learning {
            c"keyin_pinyin"
        } else {
            c"keyin_pinyin_private"
        };
        if unsafe { (self.api().select_schema)(self.id, schema.as_ptr()) } != 0 {
            self.learning = learning;
        }
    }

    pub fn input(&self) -> String {
        text(unsafe { (self.api().get_input)(self.id) })
    }

    pub fn caret(&self) -> usize {
        unsafe { (self.api().get_caret_pos)(self.id) }
    }

    pub fn clear(&mut self) {
        unsafe { (self.api().clear_composition)(self.id) }
    }

    pub fn process_key(&mut self, key: i32) -> bool {
        unsafe { (self.api().process_key)(self.id, key, 0) != 0 }
    }

    pub fn set_input(&mut self, raw: &str) -> bool {
        if raw.len() > 128 {
            return false;
        }
        let normalized = raw.to_lowercase();
        let old = self.input();
        if old == normalized {
            return true;
        }
        let start = if normalized.starts_with(&old) && self.caret() == old.len() {
            old.len()
        } else {
            self.clear();
            0
        };
        for key in normalized[start..].bytes() {
            if !(key.is_ascii_lowercase() || key == b'\'') || !self.process_key(key as i32) {
                self.clear();
                return false;
            }
        }
        true
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        let api = self.api();
        // SAFETY: all-zero is a valid empty context for librime to fill.
        let mut context: Context = unsafe { std::mem::zeroed() };
        context.data_size = struct_size::<Context>();
        if unsafe { (api.get_context)(self.id, &mut context) } == 0 {
            return None;
        }
        let preedit = text(context.composition.preedit);
        // Rime offsets are UTF-8 bytes; host text APIs use UTF-16 positions.
        let bytes = (context.composition.cursor_pos.max(0) as usize).min(preedit.len());
        let caret = preedit
            .get(..bytes)
            .map_or(preedit.encode_utf16().count(), |prefix| {
                prefix.encode_utf16().count()
            });
        unsafe { (api.free_context)(&mut context) };
        let mut choices = Vec::new();
        let mut iterator: CandidateListIterator = unsafe { std::mem::zeroed() };
        if unsafe { (api.candidate_list_begin)(self.id, &mut iterator) } != 0 {
            while choices.len() < 90 && unsafe { (api.candidate_list_next)(&mut iterator) } != 0 {
                if !iterator.candidate.text.is_null() {
                    choices.push(text(iterator.candidate.text));
                }
            }
            unsafe { (api.candidate_list_end)(&mut iterator) };
        }
        Some(Snapshot {
            preedit,
            caret,
            choices,
        })
    }

    /// Returns the committed text, or `None` when only one segment was selected.
    pub fn select_candidate(&mut self, index: usize) -> Option<String> {
        let api = self.api();
        if unsafe { (api.select_candidate)(self.id, index) } == 0 {
            return None;
        }
        let mut commit = Commit {
            data_size: struct_size::<Commit>(),
            text: ptr::null_mut(),
        };
        if unsafe { (api.get_commit)(self.id, &mut commit) } == 0 {
            return None;
        }
        let committed = text(commit.text);
        unsafe { (api.free_commit)(&mut commit) };
        Some(committed)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.api().destroy_session)(self.id) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn api_table_matches_rime_api_h() {
        // Indices of the function pointers in RimeApi, after the padded data_size.
        let slot = |index: usize| 8 + index * 8;
        assert_eq!(std::mem::offset_of!(RimeApi, create_session), slot(13));
        assert_eq!(std::mem::offset_of!(RimeApi, process_key), slot(18));
        assert_eq!(std::mem::offset_of!(RimeApi, get_commit), slot(21));
        assert_eq!(std::mem::offset_of!(RimeApi, set_option), slot(27));
        assert_eq!(std::mem::offset_of!(RimeApi, select_schema), slot(34));
        assert_eq!(std::mem::offset_of!(RimeApi, get_input), slot(69));
        assert_eq!(std::mem::offset_of!(RimeApi, get_version), slot(72));
        assert_eq!(std::mem::offset_of!(RimeApi, candidate_list_end), slot(77));
        assert_eq!(std::mem::size_of::<Context>(), 88);
        assert_eq!(std::mem::size_of::<Traits>(), 96);
    }
}
