use crate::abi::*;
use crate::clipper;
use crate::log;
use crate::palette;
use crate::state::{new_shared, SharedState};
use crate::surface;
use crate::window;
use core::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

const VTABLE_LEN: usize = 23;

#[repr(C)]
pub struct DirectDraw {
    vtable: *const usize,
    refs: AtomicU32,
    state: SharedState,
}

unsafe fn from_this(this: *mut c_void) -> *mut DirectDraw {
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
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_IDIRECTDRAW) {
        add_ref(this);
        *output = this;
        return S_OK;
    }
    log::trace("IDirectDraw::QueryInterface unsupported IID");
    E_NOINTERFACE
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> Ulong {
    let object = from_this(this);
    if object.is_null() {
        return 0;
    }
    (*object).refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> Ulong {
    let object = from_this(this);
    if object.is_null() {
        return 0;
    }
    let remaining = (*object).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        window::leave_borderless(&(*object).state);
        drop(Box::from_raw(object));
        log::line("IDirectDraw object released");
    }
    remaining
}

unsafe extern "system" fn compact(_this: *mut c_void) -> Hresult {
    DD_OK
}

unsafe extern "system" fn create_clipper(
    _this: *mut c_void,
    _flags: Dword,
    output: *mut *mut c_void,
    outer: *mut c_void,
) -> Hresult {
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    clipper::create(output)
}

unsafe extern "system" fn create_palette(
    _this: *mut c_void,
    flags: Dword,
    entries: *mut PaletteEntry,
    output: *mut *mut c_void,
    outer: *mut c_void,
) -> Hresult {
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    palette::create(flags, entries, output)
}

unsafe extern "system" fn create_surface(
    this: *mut c_void,
    desc: *mut DdSurfaceDesc,
    output: *mut *mut c_void,
    outer: *mut c_void,
) -> Hresult {
    if output.is_null() || desc.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }

    let object = from_this(this);
    let state = (*object).state.clone();
    let state_snapshot = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let caps = (*desc).caps.caps;
    let primary = caps & DDSCAPS_PRIMARYSURFACE != 0;
    let width = if primary || (*desc).flags & DDSD_WIDTH == 0 {
        state_snapshot.logical_width
    } else {
        (*desc).width
    };
    let height = if primary || (*desc).flags & DDSD_HEIGHT == 0 {
        state_snapshot.logical_height
    } else {
        (*desc).height
    };
    let bpp = if (*desc).flags & DDSD_PIXELFORMAT != 0 && (*desc).pixel_format.rgb_bit_count != 0 {
        (*desc).pixel_format.rgb_bit_count
    } else {
        state_snapshot.bpp
    };
    drop(state_snapshot);

    let surface_ptr = match surface::create(state.clone(), width, height, bpp, caps, primary) {
        Ok(pointer) => pointer,
        Err(hr) => return hr,
    };

    if primary
        && (((*desc).flags & DDSD_BACKBUFFERCOUNT != 0 && (*desc).back_buffer_count > 0)
            || caps & (DDSCAPS_FLIP | DDSCAPS_COMPLEX) != 0)
    {
        let back_caps = DDSCAPS_BACKBUFFER | DDSCAPS_SYSTEMMEMORY;
        match surface::create(state, width, height, bpp, back_caps, false) {
            Ok(back) => surface::attach(surface_ptr, back),
            Err(hr) => {
                let _ = surface::release(surface_ptr);
                return hr;
            }
        }
    }

    *output = surface_ptr;
    log::trace(&format!(
        "IDirectDraw::CreateSurface {}x{}x{} caps=0x{caps:08X} primary={primary}",
        width, height, bpp
    ));
    DD_OK
}

unsafe extern "system" fn duplicate_surface(
    _this: *mut c_void,
    source: *mut c_void,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    match surface::duplicate(source) {
        Ok(pointer) => {
            *output = pointer;
            DD_OK
        }
        Err(hr) => hr,
    }
}

unsafe extern "system" fn enum_display_modes(
    this: *mut c_void,
    _flags: Dword,
    filter: *mut DdSurfaceDesc,
    context: *mut c_void,
    callback: *mut c_void,
) -> Hresult {
    if callback.is_null() {
        return E_POINTER;
    }
    let object = from_this(this);
    let state = (*object).state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut desc = DdSurfaceDesc::default();
    desc.flags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT | DDSD_CAPS;
    desc.width = if !filter.is_null() && (*filter).width != 0 {
        (*filter).width
    } else {
        state.logical_width
    };
    desc.height = if !filter.is_null() && (*filter).height != 0 {
        (*filter).height
    } else {
        state.logical_height
    };
    let bpp = if !filter.is_null() && (*filter).pixel_format.rgb_bit_count != 0 {
        (*filter).pixel_format.rgb_bit_count
    } else {
        state.bpp
    };
    desc.pixel_format = match bpp {
        8 => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB | DDPF_PALETTEINDEXED8,
            rgb_bit_count: 8,
            ..DdPixelFormat::default()
        },
        16 => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB,
            rgb_bit_count: 16,
            r_mask: 0xF800,
            g_mask: 0x07E0,
            b_mask: 0x001F,
            ..DdPixelFormat::default()
        },
        _ => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB,
            rgb_bit_count: bpp,
            r_mask: 0x00FF_0000,
            g_mask: 0x0000_FF00,
            b_mask: 0x0000_00FF,
            ..DdPixelFormat::default()
        },
    };
    drop(state);
    let function: unsafe extern "system" fn(*mut DdSurfaceDesc, *mut c_void) -> Hresult = mem::transmute(callback);
    let _ = function(&mut desc, context);
    DD_OK
}

unsafe extern "system" fn enum_surfaces(
    _this: *mut c_void,
    _flags: Dword,
    _desc: *mut DdSurfaceDesc,
    _context: *mut c_void,
    _callback: *mut c_void,
) -> Hresult {
    DD_OK
}

unsafe extern "system" fn flip_to_gdi_surface(_this: *mut c_void) -> Hresult {
    DD_OK
}

unsafe extern "system" fn get_caps(
    _this: *mut c_void,
    driver_caps: *mut c_void,
    hel_caps: *mut c_void,
) -> Hresult {
    for pointer in [driver_caps, hel_caps] {
        if !pointer.is_null() {
            let size = *(pointer.cast::<u32>()) as usize;
            if size >= 4 && size <= 1024 {
                ptr::write_bytes(pointer.cast::<u8>().add(4), 0, size - 4);
            }
        }
    }
    DD_OK
}

unsafe extern "system" fn get_display_mode(
    this: *mut c_void,
    output: *mut DdSurfaceDesc,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    let object = from_this(this);
    let state = (*object).state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut desc = DdSurfaceDesc::default();
    desc.flags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT | DDSD_CAPS;
    desc.width = state.logical_width;
    desc.height = state.logical_height;
    desc.pixel_format.size = mem::size_of::<DdPixelFormat>() as u32;
    desc.pixel_format.flags = DDPF_RGB;
    desc.pixel_format.rgb_bit_count = state.bpp;
    if state.bpp == 16 {
        desc.pixel_format.r_mask = 0xF800;
        desc.pixel_format.g_mask = 0x07E0;
        desc.pixel_format.b_mask = 0x001F;
    } else if state.bpp == 8 {
        desc.pixel_format.flags |= DDPF_PALETTEINDEXED8;
    } else {
        desc.pixel_format.r_mask = 0x00FF_0000;
        desc.pixel_format.g_mask = 0x0000_FF00;
        desc.pixel_format.b_mask = 0x0000_00FF;
    }
    *output = desc;
    DD_OK
}

unsafe extern "system" fn get_four_cc_codes(
    _this: *mut c_void,
    count: *mut Dword,
    _codes: *mut Dword,
) -> Hresult {
    if count.is_null() {
        return E_POINTER;
    }
    *count = 0;
    DD_OK
}

unsafe extern "system" fn get_gdi_surface(
    _this: *mut c_void,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    let primary = surface::primary_pointer();
    if primary.is_null() {
        *output = ptr::null_mut();
        return E_FAIL;
    }
    surface::add_ref(primary);
    *output = primary;
    DD_OK
}

unsafe extern "system" fn get_monitor_frequency(
    _this: *mut c_void,
    frequency: *mut Dword,
) -> Hresult {
    if frequency.is_null() {
        return E_POINTER;
    }
    *frequency = 60;
    DD_OK
}

unsafe extern "system" fn get_scan_line(_this: *mut c_void, line: *mut Dword) -> Hresult {
    if line.is_null() {
        return E_POINTER;
    }
    *line = 0;
    DD_OK
}

unsafe extern "system" fn get_vertical_blank_status(
    _this: *mut c_void,
    status: *mut Bool,
) -> Hresult {
    if status.is_null() {
        return E_POINTER;
    }
    *status = FALSE;
    DD_OK
}

unsafe extern "system" fn initialize(_this: *mut c_void, _guid: *mut Guid) -> Hresult {
    DD_OK
}

unsafe extern "system" fn restore_display_mode(this: *mut c_void) -> Hresult {
    let object = from_this(this);
    window::leave_borderless(&(*object).state);
    DD_OK
}

unsafe extern "system" fn set_cooperative_level(
    this: *mut c_void,
    hwnd: Hwnd,
    flags: Dword,
) -> Hresult {
    let object = from_this(this);
    {
        let mut state = (*object).state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.hwnd = hwnd as usize;
        state.cooperative_flags = flags;
        state.fullscreen_requested = flags & (DDSCL_EXCLUSIVE | DDSCL_FULLSCREEN) != 0;
    }
    window::install(hwnd, &(*object).state);
    let fullscreen = flags & (DDSCL_EXCLUSIVE | DDSCL_FULLSCREEN) != 0;
    if fullscreen {
        window::enter_borderless(&(*object).state);
    } else if flags & DDSCL_NORMAL != 0 {
        window::leave_borderless(&(*object).state);
    }
    log::line(&format!(
        "IDirectDraw::SetCooperativeLevel hwnd=0x{:08X} flags=0x{flags:08X}",
        hwnd as usize
    ));
    DD_OK
}

unsafe extern "system" fn set_display_mode(
    this: *mut c_void,
    width: Dword,
    height: Dword,
    bpp: Dword,
) -> Hresult {
    let object = from_this(this);
    let fullscreen = {
        let mut state = (*object).state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.logical_width = width.max(1);
        state.logical_height = height.max(1);
        state.bpp = match bpp {
            8 | 16 | 24 | 32 => bpp,
            _ => 32,
        };
        state.fullscreen_requested
    };
    if fullscreen {
        window::enter_borderless(&(*object).state);
    }
    log::line(&format!(
        "IDirectDraw::SetDisplayMode logical={}x{}x{}; desktop mode unchanged",
        width, height, bpp
    ));
    DD_OK
}

unsafe extern "system" fn wait_for_vertical_blank(
    _this: *mut c_void,
    _flags: Dword,
    _event: Handle,
) -> Hresult {
    Sleep(1);
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
                compact as *const () as usize,
                create_clipper as *const () as usize,
                create_palette as *const () as usize,
                create_surface as *const () as usize,
                duplicate_surface as *const () as usize,
                enum_display_modes as *const () as usize,
                enum_surfaces as *const () as usize,
                flip_to_gdi_surface as *const () as usize,
                get_caps as *const () as usize,
                get_display_mode as *const () as usize,
                get_four_cc_codes as *const () as usize,
                get_gdi_surface as *const () as usize,
                get_monitor_frequency as *const () as usize,
                get_scan_line as *const () as usize,
                get_vertical_blank_status as *const () as usize,
                initialize as *const () as usize,
                restore_display_mode as *const () as usize,
                set_cooperative_level as *const () as usize,
                set_display_mode as *const () as usize,
                wait_for_vertical_blank as *const () as usize,
            ])
        })
        .as_ptr()
}

pub unsafe fn create(output: *mut *mut c_void) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let object = Box::new(DirectDraw {
        vtable: vtable(),
        refs: AtomicU32::new(1),
        state: new_shared(),
    });
    let pointer = Box::into_raw(object);
    *output = pointer.cast();
    log::line(&format!(
        "created Rust IDirectDraw v1 object=0x{:08X}",
        pointer as usize
    ));
    DD_OK
}
