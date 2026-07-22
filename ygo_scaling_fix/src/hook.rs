use crate::abi::*;
use crate::ddraw;
use crate::log;
use core::ffi::c_void;
use retour::static_detour;
use std::ptr;
use std::sync::OnceLock;

static_detour! {
    static CoCreateInstanceDetour: unsafe extern "system" fn(
        *const Guid,
        *mut c_void,
        Dword,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult;
}

static INSTALL_RESULT: OnceLock<bool> = OnceLock::new();

unsafe extern "system" fn detoured_cocreateinstance(
    rclsid: *const Guid,
    outer: *mut c_void,
    context: Dword,
    riid: *const Guid,
    output: *mut *mut c_void,
) -> Hresult {
    if guid_eq(rclsid, &CLSID_DIRECTDRAW) && guid_eq(riid, &IID_IDIRECTDRAW) {
        if !outer.is_null() {
            return CLASS_E_NOAGGREGATION;
        }
        log::line("intercepted CoCreateInstance(CLSID_DirectDraw, IID_IDirectDraw)");
        return ddraw::create(output);
    }

    CoCreateInstanceDetour.call(rclsid, outer, context, riid, output)
}

pub unsafe fn install() -> bool {
    *INSTALL_RESULT.get_or_init(|| {
        if CoCreateInstanceDetour
            .initialize(
                CoCreateInstance as CoCreateInstanceFn,
                |rclsid, outer, context, riid, output| unsafe {
                    detoured_cocreateinstance(rclsid, outer, context, riid, output)
                },
            )
            .is_err()
        {
            log::line("retour failed to initialize CoCreateInstance static detour");
            return false;
        }
        if CoCreateInstanceDetour.enable().is_err() {
            log::line("retour failed to enable CoCreateInstance static detour");
            return false;
        }
        log::line("retour static detour enabled for ole32!CoCreateInstance");
        true
    })
}

pub unsafe extern "system" fn bootstrap_thread(_parameter: *mut c_void) -> Dword {
    log::line("ygo_fix_retour_v12 loaded; deferred bootstrap thread started");
    let _ = install();
    0
}

pub unsafe fn spawn_bootstrap_thread() {
    let handle = CreateThread(
        ptr::null_mut(),
        0,
        Some(bootstrap_thread),
        ptr::null_mut(),
        0,
        ptr::null_mut(),
    );
    if !handle.is_null() {
        let _ = CloseHandle(handle);
    }
}
