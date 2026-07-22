use crate::abi::*;
use core::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::OnceLock;

const VTABLE_LEN: usize = 9;

#[repr(C)]
pub struct Clipper {
    vtable: *const usize,
    refs: AtomicU32,
    hwnd: AtomicUsize,
    changed: AtomicBool,
}

unsafe fn from_this(this: *mut c_void) -> *mut Clipper {
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
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_IDIRECTDRAWCLIPPER) {
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

unsafe extern "system" fn get_clip_list(
    _this: *mut c_void,
    _rect: *mut Rect,
    _data: *mut RgnData,
    size: *mut Dword,
) -> Hresult {
    if size.is_null() {
        return E_POINTER;
    }
    *size = 0;
    DD_OK
}

unsafe extern "system" fn get_hwnd(this: *mut c_void, hwnd: *mut Hwnd) -> Hresult {
    if hwnd.is_null() {
        return E_POINTER;
    }
    *hwnd = (*from_this(this)).hwnd.load(Ordering::Acquire) as Hwnd;
    DD_OK
}

unsafe extern "system" fn initialize(
    _this: *mut c_void,
    _ddraw: *mut c_void,
    _flags: Dword,
) -> Hresult {
    DD_OK
}

unsafe extern "system" fn is_clip_list_changed(this: *mut c_void, changed: *mut Bool) -> Hresult {
    if changed.is_null() {
        return E_POINTER;
    }
    *changed = if (*from_this(this)).changed.swap(false, Ordering::AcqRel) {
        TRUE
    } else {
        FALSE
    };
    DD_OK
}

unsafe extern "system" fn set_clip_list(
    this: *mut c_void,
    _data: *mut RgnData,
    _flags: Dword,
) -> Hresult {
    (*from_this(this)).changed.store(true, Ordering::Release);
    DD_OK
}

unsafe extern "system" fn set_hwnd(this: *mut c_void, _flags: Dword, hwnd: Hwnd) -> Hresult {
    (*from_this(this)).hwnd.store(hwnd as usize, Ordering::Release);
    (*from_this(this)).changed.store(true, Ordering::Release);
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
                get_clip_list as *const () as usize,
                get_hwnd as *const () as usize,
                initialize as *const () as usize,
                is_clip_list_changed as *const () as usize,
                set_clip_list as *const () as usize,
                set_hwnd as *const () as usize,
            ])
        })
        .as_ptr()
}

pub unsafe fn create(output: *mut *mut c_void) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let object = Box::new(Clipper {
        vtable: vtable(),
        refs: AtomicU32::new(1),
        hwnd: AtomicUsize::new(0),
        changed: AtomicBool::new(false),
    });
    *output = Box::into_raw(object).cast();
    DD_OK
}
