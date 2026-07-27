#![allow(dead_code)]

use core::ffi::{c_char, c_void};

pub type Bool = i32;
pub type Dword = u32;
pub type Uint = u32;
pub type Ulong = u32;
pub type Long = i32;
pub type Hresult = i32;
pub type Lresult = isize;
pub type Wparam = usize;
pub type Lparam = isize;
pub type Handle = *mut c_void;
pub type Hmodule = *mut c_void;
pub type Hwnd = *mut c_void;
pub type Hdc = *mut c_void;
pub type Hgdiobj = *mut c_void;
pub type Hbitmap = *mut c_void;
pub type Hbrush = *mut c_void;
pub type Hrgn = *mut c_void;

pub const TRUE: Bool = 1;
pub const FALSE: Bool = 0;
pub const S_OK: Hresult = 0;
pub const DD_OK: Hresult = 0;
pub const E_NOTIMPL: Hresult = 0x8000_4001u32 as i32;
pub const E_NOINTERFACE: Hresult = 0x8000_4002u32 as i32;
pub const E_POINTER: Hresult = 0x8000_4003u32 as i32;
pub const E_FAIL: Hresult = 0x8000_4005u32 as i32;
pub const E_INVALIDARG: Hresult = 0x8007_0057u32 as i32;
pub const CLASS_E_NOAGGREGATION: Hresult = 0x8004_0110u32 as i32;

pub const DLL_PROCESS_ATTACH: Dword = 1;
pub const DLL_PROCESS_DETACH: Dword = 0;

pub const DDSCL_FULLSCREEN: Dword = 0x0000_0001;
pub const DDSCL_ALLOWREBOOT: Dword = 0x0000_0002;
pub const DDSCL_NOWINDOWCHANGES: Dword = 0x0000_0004;
pub const DDSCL_NORMAL: Dword = 0x0000_0008;
pub const DDSCL_EXCLUSIVE: Dword = 0x0000_0010;

pub const DDSD_CAPS: Dword = 0x0000_0001;
pub const DDSD_HEIGHT: Dword = 0x0000_0002;
pub const DDSD_WIDTH: Dword = 0x0000_0004;
pub const DDSD_PITCH: Dword = 0x0000_0008;
pub const DDSD_BACKBUFFERCOUNT: Dword = 0x0000_0020;
pub const DDSD_LPSURFACE: Dword = 0x0000_0800;
pub const DDSD_PIXELFORMAT: Dword = 0x0000_1000;

pub const DDSCAPS_BACKBUFFER: Dword = 0x0000_0004;
pub const DDSCAPS_COMPLEX: Dword = 0x0000_0008;
pub const DDSCAPS_FLIP: Dword = 0x0000_0010;
pub const DDSCAPS_OFFSCREENPLAIN: Dword = 0x0000_0040;
pub const DDSCAPS_PRIMARYSURFACE: Dword = 0x0000_0200;
pub const DDSCAPS_SYSTEMMEMORY: Dword = 0x0000_0800;
pub const DDSCAPS_VIDEOMEMORY: Dword = 0x0000_4000;

pub const DDPF_PALETTEINDEXED8: Dword = 0x0000_0020;
pub const DDPF_RGB: Dword = 0x0000_0040;

pub const DDBLT_COLORFILL: Dword = 0x0000_0400;
pub const DDBLT_KEYSRC: Dword = 0x0000_8000;
pub const DDBLT_KEYSRCOVERRIDE: Dword = 0x0001_0000;
pub const DDBLTFAST_SRCCOLORKEY: Dword = 0x0000_0001;

pub const DDCKEY_SRCBLT: Dword = 0x0000_0008;

pub const BI_RGB: Dword = 0;
pub const BI_BITFIELDS: Dword = 3;
pub const DIB_RGB_COLORS: Uint = 0;
pub const SRCCOPY: Dword = 0x00CC_0020;
pub const BLACKNESS: Dword = 0x0000_0042;
pub const COLORONCOLOR: i32 = 3;
pub const HALFTONE: i32 = 4;
pub const MM_TEXT: i32 = 1;
pub const GM_COMPATIBLE: i32 = 1;
pub const GM_ADVANCED: i32 = 2;

pub const GWL_STYLE: i32 = -16;
pub const GWL_EXSTYLE: i32 = -20;
pub const GWL_WNDPROC: i32 = -4;
pub const WS_POPUP: Long = 0x8000_0000u32 as i32;
pub const WS_VISIBLE: Long = 0x1000_0000;
pub const SWP_NOSIZE: Uint = 0x0001;
pub const SWP_NOMOVE: Uint = 0x0002;
pub const SWP_NOZORDER: Uint = 0x0004;
pub const SWP_NOACTIVATE: Uint = 0x0010;
pub const SWP_NOSENDCHANGING: Uint = 0x0400;
pub const SWP_FRAMECHANGED: Uint = 0x0020;
pub const SWP_SHOWWINDOW: Uint = 0x0040;
pub const MONITOR_DEFAULTTONEAREST: Dword = 2;

pub const WM_PAINT: Uint = 0x000F;
pub const WM_WINDOWPOSCHANGING: Uint = 0x0046;
pub const WM_WINDOWPOSCHANGED: Uint = 0x0047;
pub const WM_ERASEBKGND: Uint = 0x0014;
pub const WM_NCDESTROY: Uint = 0x0082;
pub const WM_MOUSEMOVE: Uint = 0x0200;
pub const WM_LBUTTONDOWN: Uint = 0x0201;
pub const WM_LBUTTONUP: Uint = 0x0202;
pub const WM_LBUTTONDBLCLK: Uint = 0x0203;
pub const WM_RBUTTONDOWN: Uint = 0x0204;
pub const WM_RBUTTONUP: Uint = 0x0205;
pub const WM_RBUTTONDBLCLK: Uint = 0x0206;
pub const WM_MBUTTONDOWN: Uint = 0x0207;
pub const WM_MBUTTONUP: Uint = 0x0208;
pub const WM_MBUTTONDBLCLK: Uint = 0x0209;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub const CLSID_DIRECTDRAW: Guid = Guid {
    data1: 0xD7B70EE0,
    data2: 0x4340,
    data3: 0x11CF,
    data4: [0xB0, 0x63, 0x00, 0x20, 0xAF, 0xC2, 0xCD, 0x35],
};

pub const IID_IUNKNOWN: Guid = Guid {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

pub const IID_IDIRECTDRAW: Guid = Guid {
    data1: 0x6C14DB80,
    data2: 0xA733,
    data3: 0x11CE,
    data4: [0xA5, 0x21, 0x00, 0x20, 0xAF, 0x0B, 0xE5, 0x60],
};

pub const IID_IDIRECTDRAWSURFACE: Guid = Guid {
    data1: 0x6C14DB81,
    data2: 0xA733,
    data3: 0x11CE,
    data4: [0xA5, 0x21, 0x00, 0x20, 0xAF, 0x0B, 0xE5, 0x60],
};

pub const IID_IDIRECTDRAWPALETTE: Guid = Guid {
    data1: 0x6C14DB84,
    data2: 0xA733,
    data3: 0x11CE,
    data4: [0xA5, 0x21, 0x00, 0x20, 0xAF, 0x0B, 0xE5, 0x60],
};

pub const IID_IDIRECTDRAWCLIPPER: Guid = Guid {
    data1: 0x6C14DB85,
    data2: 0xA733,
    data3: 0x11CE,
    data4: [0xA5, 0x21, 0x00, 0x20, 0xAF, 0x0B, 0xE5, 0x60],
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: Long,
    pub y: Long,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Xform {
    pub e_m11: f32,
    pub e_m12: f32,
    pub e_m21: f32,
    pub e_m22: f32,
    pub e_dx: f32,
    pub e_dy: f32,
}

impl Default for Xform {
    fn default() -> Self {
        Self {
            e_m11: 1.0,
            e_m12: 0.0,
            e_m21: 0.0,
            e_m22: 1.0,
            e_dx: 0.0,
            e_dy: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub left: Long,
    pub top: Long,
    pub right: Long,
    pub bottom: Long,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right.saturating_sub(self.left)
    }
    pub fn height(self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowPos {
    pub hwnd: Hwnd,
    pub insert_after: Hwnd,
    pub x: Long,
    pub y: Long,
    pub cx: Long,
    pub cy: Long,
    pub flags: Uint,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MonitorInfoA {
    pub cb_size: Dword,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
    pub dw_flags: Dword,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PaintStruct {
    pub hdc: Hdc,
    pub erase: Bool,
    pub paint: Rect,
    pub restore: Bool,
    pub inc_update: Bool,
    pub reserved: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct BitmapInfoHeader {
    pub bi_size: Dword,
    pub bi_width: Long,
    pub bi_height: Long,
    pub bi_planes: u16,
    pub bi_bit_count: u16,
    pub bi_compression: Dword,
    pub bi_size_image: Dword,
    pub bi_x_pels_per_meter: Long,
    pub bi_y_pels_per_meter: Long,
    pub bi_clr_used: Dword,
    pub bi_clr_important: Dword,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RgbQuad {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PaletteEntry {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub flags: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DdColorKey {
    pub low: Dword,
    pub high: Dword,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DdPixelFormat {
    pub size: Dword,
    pub flags: Dword,
    pub four_cc: Dword,
    pub rgb_bit_count: Dword,
    pub r_mask: Dword,
    pub g_mask: Dword,
    pub b_mask: Dword,
    pub a_mask: Dword,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DdCaps {
    pub caps: Dword,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DdSurfaceDesc {
    pub size: Dword,
    pub flags: Dword,
    pub height: Dword,
    pub width: Dword,
    pub pitch: Long,
    pub back_buffer_count: Dword,
    pub mip_map_count: Dword,
    pub alpha_bit_depth: Dword,
    pub reserved: Dword,
    pub surface: *mut c_void,
    pub ck_dest_overlay: DdColorKey,
    pub ck_dest_blt: DdColorKey,
    pub ck_src_overlay: DdColorKey,
    pub ck_src_blt: DdColorKey,
    pub pixel_format: DdPixelFormat,
    pub caps: DdCaps,
}

impl Default for DdSurfaceDesc {
    fn default() -> Self {
        Self {
            size: core::mem::size_of::<Self>() as u32,
            flags: 0,
            height: 0,
            width: 0,
            pitch: 0,
            back_buffer_count: 0,
            mip_map_count: 0,
            alpha_bit_depth: 0,
            reserved: 0,
            surface: core::ptr::null_mut(),
            ck_dest_overlay: DdColorKey::default(),
            ck_dest_blt: DdColorKey::default(),
            ck_src_overlay: DdColorKey::default(),
            ck_src_blt: DdColorKey::default(),
            pixel_format: DdPixelFormat::default(),
            caps: DdCaps::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DdBltFxPrefix {
    pub before_fill: [Dword; 20],
    pub fill_color: Dword,
    pub dest_color_key: DdColorKey,
    pub src_color_key: DdColorKey,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RgnDataHeader {
    pub size: Dword,
    pub region_type: Dword,
    pub count: Dword,
    pub region_size: Dword,
    pub bound: Rect,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RgnData {
    pub header: RgnDataHeader,
    pub buffer: [u8; 1],
}

pub type DirectDrawCreateFn = unsafe extern "system" fn(
    *const Guid,
    *mut *mut c_void,
    *mut c_void,
) -> Hresult;

pub type DirectDrawCreateExFn = unsafe extern "system" fn(
    *const Guid,
    *mut *mut c_void,
    *const Guid,
    *mut c_void,
) -> Hresult;

pub type CoCreateInstanceFn = unsafe extern "system" fn(
    *const Guid,
    *mut c_void,
    Dword,
    *const Guid,
    *mut *mut c_void,
) -> Hresult;

pub type ThreadProc = unsafe extern "system" fn(*mut c_void) -> Dword;

#[link(name = "ole32")]
extern "system" {
    pub fn CoCreateInstance(
        rclsid: *const Guid,
        outer: *mut c_void,
        context: Dword,
        riid: *const Guid,
        output: *mut *mut c_void,
    ) -> Hresult;
}

#[link(name = "kernel32")]
extern "system" {
    pub fn CreateThread(
        attributes: *mut c_void,
        stack_size: usize,
        start: Option<ThreadProc>,
        parameter: *mut c_void,
        creation_flags: Dword,
        thread_id: *mut Dword,
    ) -> Handle;
    pub fn CloseHandle(handle: Handle) -> Bool;
    pub fn DisableThreadLibraryCalls(module: Hmodule) -> Bool;
    pub fn GetModuleFileNameW(module: Hmodule, buffer: *mut u16, size: Dword) -> Dword;
    pub fn GetTickCount() -> Dword;
    pub fn GetModuleHandleA(name: *const c_char) -> Hmodule;
    pub fn LoadLibraryA(name: *const c_char) -> Hmodule;
    pub fn GetProcAddress(module: Hmodule, name: *const c_char) -> *mut c_void;
    pub fn Sleep(milliseconds: Dword);
}

#[link(name = "user32")]
extern "system" {
    pub fn CallWindowProcA(
        previous: *const c_void,
        hwnd: Hwnd,
        message: Uint,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult;
    pub fn DefWindowProcA(hwnd: Hwnd, message: Uint, wparam: Wparam, lparam: Lparam) -> Lresult;
    pub fn BeginPaint(hwnd: Hwnd, paint: *mut PaintStruct) -> Hdc;
    pub fn EndPaint(hwnd: Hwnd, paint: *const PaintStruct) -> Bool;
    pub fn GetClientRect(hwnd: Hwnd, rect: *mut Rect) -> Bool;
    pub fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> Bool;
    pub fn GetMenu(hwnd: Hwnd) -> Handle;
    pub fn AdjustWindowRectEx(
        rect: *mut Rect,
        style: Dword,
        has_menu: Bool,
        ex_style: Dword,
    ) -> Bool;
    pub fn GetDC(hwnd: Hwnd) -> Hdc;
    pub fn ReleaseDC(hwnd: Hwnd, hdc: Hdc) -> i32;
    pub fn GetWindowLongA(hwnd: Hwnd, index: i32) -> Long;
    pub fn SetWindowLongA(hwnd: Hwnd, index: i32, value: Long) -> Long;
    pub fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> Bool;
    pub fn SetWindowPos(
        hwnd: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: Uint,
    ) -> Bool;
    pub fn MonitorFromWindow(hwnd: Hwnd, flags: Dword) -> Handle;
    pub fn GetMonitorInfoA(monitor: Handle, info: *mut MonitorInfoA) -> Bool;
    pub fn FillRect(hdc: Hdc, rect: *const Rect, brush: Hbrush) -> i32;
    pub fn GetSysColorBrush(index: i32) -> Hbrush;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    pub fn CreateCompatibleBitmap(hdc: Hdc, width: i32, height: i32) -> Hbitmap;
    pub fn DeleteDC(hdc: Hdc) -> Bool;
    pub fn CreateDIBSection(
        hdc: Hdc,
        info: *const c_void,
        usage: Uint,
        bits: *mut *mut c_void,
        section: Handle,
        offset: Dword,
    ) -> Hbitmap;
    pub fn SelectObject(hdc: Hdc, object: Hgdiobj) -> Hgdiobj;
    pub fn DeleteObject(object: Hgdiobj) -> Bool;
    pub fn SetDIBColorTable(hdc: Hdc, start: Uint, count: Uint, colors: *const RgbQuad) -> Uint;
    pub fn SaveDC(hdc: Hdc) -> i32;
    pub fn RestoreDC(hdc: Hdc, saved_dc: i32) -> Bool;
    pub fn GetMapMode(hdc: Hdc) -> i32;
    pub fn SetGraphicsMode(hdc: Hdc, mode: i32) -> i32;
    pub fn SetWorldTransform(hdc: Hdc, transform: *const Xform) -> Bool;
    pub fn SetMapMode(hdc: Hdc, mode: i32) -> i32;
    pub fn GetWindowOrgEx(hdc: Hdc, point: *mut Point) -> Bool;
    pub fn GetViewportOrgEx(hdc: Hdc, point: *mut Point) -> Bool;
    pub fn GetWindowExtEx(hdc: Hdc, size: *mut Point) -> Bool;
    pub fn GetViewportExtEx(hdc: Hdc, size: *mut Point) -> Bool;
    pub fn SetWindowOrgEx(hdc: Hdc, x: i32, y: i32, previous: *mut Point) -> Bool;
    pub fn SetViewportOrgEx(hdc: Hdc, x: i32, y: i32, previous: *mut Point) -> Bool;
    pub fn SetStretchBltMode(hdc: Hdc, mode: i32) -> i32;
    pub fn SetBrushOrgEx(hdc: Hdc, x: i32, y: i32, previous: *mut Point) -> Bool;
    pub fn BitBlt(
        dst: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src: Hdc,
        src_x: i32,
        src_y: i32,
        rop: Dword,
    ) -> Bool;
    pub fn StretchDIBits(
        dst: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src_x: i32,
        src_y: i32,
        src_width: i32,
        src_height: i32,
        bits: *const c_void,
        info: *const c_void,
        usage: Uint,
        rop: Dword,
    ) -> i32;
    pub fn StretchBlt(
        dst: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src: Hdc,
        src_x: i32,
        src_y: i32,
        src_width: i32,
        src_height: i32,
        rop: Dword,
    ) -> Bool;
    pub fn PatBlt(hdc: Hdc, x: i32, y: i32, width: i32, height: i32, rop: Dword) -> Bool;
}

pub fn guid_eq(pointer: *const Guid, expected: &Guid) -> bool {
    if pointer.is_null() {
        return false;
    }
    unsafe { *pointer == *expected }
}

pub fn succeeded(hr: Hresult) -> bool {
    hr >= 0
}

pub fn mouse_message(message: Uint) -> bool {
    matches!(
        message,
        WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_LBUTTONDBLCLK
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_MBUTTONDBLCLK
    )
}

pub unsafe fn c_string_bytes(pointer: *const c_char) -> &'static [u8] {
    if pointer.is_null() {
        return &[];
    }
    let mut len = 0usize;
    while *pointer.add(len) != 0 {
        len += 1;
    }
    core::slice::from_raw_parts(pointer.cast::<u8>(), len)
}

// DirectDraw v1 ABI invariants for 32-bit Windows. These fail the build if a
// field change silently breaks a COM call boundary.
const _: [(); 16] = [(); core::mem::size_of::<Guid>()];
const _: [(); 16] = [(); core::mem::size_of::<Rect>()];
const _: [(); 32] = [(); core::mem::size_of::<DdPixelFormat>()];
const _: [(); 108] = [(); core::mem::size_of::<DdSurfaceDesc>()];
const _: [(); 100] = [(); core::mem::size_of::<DdBltFxPrefix>()];
