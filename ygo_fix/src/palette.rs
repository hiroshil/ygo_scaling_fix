use crate::abi::*;
use crate::surface;
use core::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const VTABLE_LEN: usize = 7;

#[repr(C)]
pub struct Palette {
    vtable: *const usize,
    refs: AtomicU32,
    flags: Dword,
    entries: Mutex<[PaletteEntry; 256]>,
}

unsafe fn from_this(this: *mut c_void) -> *mut Palette {
    this.cast()
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    riid: *const Guid,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_IDIRECTDRAWPALETTE) {
        add_ref(this);
        *output = this;
        return S_OK;
    }
    E_NOINTERFACE
}

pub unsafe extern "system" fn add_ref(this: *mut c_void) -> Ulong {
    let object = from_this(this);
    if object.is_null() {
        return 0;
    }
    (*object).refs.fetch_add(1, Ordering::Relaxed) + 1
}

pub unsafe extern "system" fn release(this: *mut c_void) -> Ulong {
    let object = from_this(this);
    if object.is_null() {
        return 0;
    }
    let remaining = (*object).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        drop(Box::from_raw(object));
    }
    remaining
}

unsafe extern "system" fn get_caps(this: *mut c_void, flags: *mut Dword) -> Hresult {
    if flags.is_null() {
        return E_POINTER;
    }
    *flags = (*from_this(this)).flags;
    DD_OK
}

unsafe extern "system" fn get_entries(
    this: *mut c_void,
    _flags: Dword,
    start: Dword,
    count: Dword,
    output: *mut PaletteEntry,
) -> Hresult {
    if output.is_null() || start >= 256 || count > 256 || start.saturating_add(count) > 256 {
        return E_INVALIDARG;
    }
    let object = from_this(this);
    let entries = (*object).entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    ptr::copy_nonoverlapping(entries.as_ptr().add(start as usize), output, count as usize);
    DD_OK
}

unsafe extern "system" fn initialize(
    this: *mut c_void,
    _ddraw: *mut c_void,
    flags: Dword,
    entries: *mut PaletteEntry,
) -> Hresult {
    let object = from_this(this);
    (*object).flags = flags;
    if !entries.is_null() {
        let mut target = (*object).entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        ptr::copy_nonoverlapping(entries, target.as_mut_ptr(), 256);
    }
    surface::palette_changed(this);
    DD_OK
}

unsafe extern "system" fn set_entries(
    this: *mut c_void,
    _flags: Dword,
    start: Dword,
    count: Dword,
    entries: *mut PaletteEntry,
) -> Hresult {
    if entries.is_null() || start >= 256 || count > 256 || start.saturating_add(count) > 256 {
        return E_INVALIDARG;
    }
    let object = from_this(this);
    {
        let mut target = (*object).entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        ptr::copy_nonoverlapping(entries, target.as_mut_ptr().add(start as usize), count as usize);
    }
    surface::palette_changed(this);
    DD_OK
}

fn vtable() -> *const usize {
    static TABLE: OnceLock<Box<[usize; VTABLE_LEN]>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            Box::new([
                query_interface as *const () as usize,
                add_ref as *const () as usize,
                release as *const () as usize,
                get_caps as *const () as usize,
                get_entries as *const () as usize,
                initialize as *const () as usize,
                set_entries as *const () as usize,
            ])
        })
        .as_ptr()
}

pub unsafe fn create(
    flags: Dword,
    entries: *mut PaletteEntry,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let mut initial = [PaletteEntry {
        red: 0,
        green: 0,
        blue: 0,
        flags: 0,
    }; 256];
    if !entries.is_null() {
        ptr::copy_nonoverlapping(entries, initial.as_mut_ptr(), 256);
    }
    let object = Box::new(Palette {
        vtable: vtable(),
        refs: AtomicU32::new(1),
        flags,
        entries: Mutex::new(initial),
    });
    *output = Box::into_raw(object).cast();
    DD_OK
}

pub unsafe fn snapshot(pointer: *mut c_void) -> Option<[PaletteEntry; 256]> {
    if pointer.is_null() {
        return None;
    }
    let object = from_this(pointer);
    Some(*(*object).entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
}

#[allow(dead_code)]
fn _abi_check() {
    let _ = mem::size_of::<Palette>();
}
