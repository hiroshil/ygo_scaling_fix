use crate::abi::*;
use crate::ddraw;
use crate::log;
use core::ffi::c_void;
use retour::static_detour;
use std::mem;
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

    static DirectDrawCreateDetour: unsafe extern "system" fn(
        *const Guid,
        *mut *mut c_void,
        *mut c_void,
    ) -> Hresult;

    static DirectDrawCreateExDetour: unsafe extern "system" fn(
        *const Guid,
        *mut *mut c_void,
        *const Guid,
        *mut c_void,
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

unsafe extern "system" fn detoured_direct_draw_create(
    guid: *const Guid,
    output: *mut *mut c_void,
    outer: *mut c_void,
) -> Hresult {
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    log::line(&format!(
        "intercepted ddraw!DirectDrawCreate guid_null={}",
        guid.is_null()
    ));
    ddraw::create(output)
}

unsafe extern "system" fn detoured_direct_draw_create_ex(
    guid: *const Guid,
    output: *mut *mut c_void,
    riid: *const Guid,
    outer: *mut c_void,
) -> Hresult {
    if guid_eq(riid, &IID_IDIRECTDRAW) {
        if !outer.is_null() {
            return CLASS_E_NOAGGREGATION;
        }
        log::line(&format!(
            "intercepted ddraw!DirectDrawCreateEx(IID_IDirectDraw) guid_null={}",
            guid.is_null()
        ));
        return ddraw::create(output);
    }

    log::line("ddraw!DirectDrawCreateEx requested unsupported interface; forwarding to system DirectDraw");
    DirectDrawCreateExDetour.call(guid, output, riid, outer)
}

unsafe fn install_cocreateinstance() -> bool {
    if CoCreateInstanceDetour
        .initialize(
            CoCreateInstance as CoCreateInstanceFn,
            |rclsid, outer, context, riid, output| unsafe {
                detoured_cocreateinstance(rclsid, outer, context, riid, output)
            },
        )
        .is_err()
    {
        log::line("retour failed to initialize ole32!CoCreateInstance detour");
        return false;
    }
    if CoCreateInstanceDetour.enable().is_err() {
        log::line("retour failed to enable ole32!CoCreateInstance detour");
        return false;
    }
    log::line("retour static detour enabled for ole32!CoCreateInstance");
    true
}

unsafe fn install_directdraw_exports() -> bool {
    let module = {
        let existing = GetModuleHandleA(b"ddraw.dll\0".as_ptr().cast());
        if existing.is_null() {
            LoadLibraryA(b"ddraw.dll\0".as_ptr().cast())
        } else {
            existing
        }
    };
    if module.is_null() {
        log::line("failed to load system ddraw.dll; DirectDrawCreate path is not intercepted");
        return false;
    }

    let create_address = GetProcAddress(module, b"DirectDrawCreate\0".as_ptr().cast());
    if create_address.is_null() {
        log::line("system ddraw.dll has no DirectDrawCreate export");
        return false;
    }
    let create: DirectDrawCreateFn = mem::transmute(create_address);
    if DirectDrawCreateDetour
        .initialize(create, |guid, output, outer| unsafe {
            detoured_direct_draw_create(guid, output, outer)
        })
        .is_err()
    {
        log::line("retour failed to initialize ddraw!DirectDrawCreate detour");
        return false;
    }
    if DirectDrawCreateDetour.enable().is_err() {
        log::line("retour failed to enable ddraw!DirectDrawCreate detour");
        return false;
    }
    log::line(&format!(
        "retour static detour enabled for ddraw!DirectDrawCreate address=0x{:08X}",
        create_address as usize
    ));

    let create_ex_address = GetProcAddress(module, b"DirectDrawCreateEx\0".as_ptr().cast());
    if create_ex_address.is_null() {
        log::line("system ddraw.dll has no DirectDrawCreateEx export; v1 path remains active");
        return true;
    }
    let create_ex: DirectDrawCreateExFn = mem::transmute(create_ex_address);
    if DirectDrawCreateExDetour
        .initialize(create_ex, |guid, output, riid, outer| unsafe {
            detoured_direct_draw_create_ex(guid, output, riid, outer)
        })
        .is_err()
    {
        log::line("retour failed to initialize ddraw!DirectDrawCreateEx detour; v1 path remains active");
        return true;
    }
    if DirectDrawCreateExDetour.enable().is_err() {
        log::line("retour failed to enable ddraw!DirectDrawCreateEx detour; v1 path remains active");
        return true;
    }
    log::line(&format!(
        "retour static detour enabled for ddraw!DirectDrawCreateEx address=0x{:08X}",
        create_ex_address as usize
    ));
    true
}

pub unsafe fn install() -> bool {
    *INSTALL_RESULT.get_or_init(|| {
        // Install the renderer path first to minimize the startup race before
        // the game makes its first DirectDrawCreate call.
        let directdraw = install_directdraw_exports();
        let cocreate = install_cocreateinstance();
        log::line(&format!(
            "hook installation summary cocreate={} directdraw_exports={}",
            cocreate, directdraw
        ));
        cocreate || directdraw
    })
}

pub unsafe extern "system" fn bootstrap_thread(_parameter: *mut c_void) -> Dword {
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
