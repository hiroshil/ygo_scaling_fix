#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]

#[cfg(not(all(target_os = "windows", target_arch = "x86")))]
compile_error!("Build only for i686-pc-windows-msvc (32-bit Windows x86).");

mod abi;
mod clipper;
mod ddraw;
mod hook;
mod log;
mod palette;
mod state;
mod surface;
mod window;

use abi::*;
use core::ffi::c_void;

#[no_mangle]
pub unsafe extern "system" fn YgoFixInitialize() -> Bool {
    if hook::install() {
        TRUE
    } else {
        FALSE
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    module: Hmodule,
    reason: Dword,
    _reserved: *mut c_void,
) -> Bool {
    match reason {
        DLL_PROCESS_ATTACH => {
            let _ = DisableThreadLibraryCalls(module);
            hook::spawn_bootstrap_thread();
        }
        // The DLL is imported for the lifetime of the process. Do not tear down
        // the detour under the loader lock during process shutdown.
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    TRUE
}
