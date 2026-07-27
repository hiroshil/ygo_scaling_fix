use crate::abi::*;
use crate::clipper;
use crate::log;
use crate::palette;
use crate::state::SharedState;
use crate::window;
use core::ffi::c_void;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

const VTABLE_LEN: usize = 36;

// The engine occasionally submits small negative source coordinates to its
// software blitter. Native DirectDraw allocations commonly have padding around
// the visible surface, while a bare DIB section can start exactly on a page
// boundary. Keep the logical surface inside a padded DIB so limited legacy
// overreads/overwrites stay inside committed memory instead of faulting.
const GUARD_PIXELS: usize = 64;
const GUARD_ROWS: usize = 64;
const MAX_SURFACE_DIMENSION: usize = 16_384;
const MAX_SURFACE_BYTES: usize = 512 * 1024 * 1024;
static PRIMARY_SURFACE: AtomicUsize = AtomicUsize::new(0);
static ALL_SURFACES: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static PRESENTER_BUFFER: Mutex<Option<PresenterBuffer>> = Mutex::new(None);
static DC_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);
static PRIMARY_RECT_TRACE_COUNT: AtomicU32 = AtomicU32::new(0);

struct PresenterBuffer {
    hwnd: usize,
    width: i32,
    height: i32,
    hdc: Hdc,
    bitmap: Hbitmap,
    old_bitmap: Hgdiobj,
}

unsafe impl Send for PresenterBuffer {}

impl Drop for PresenterBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.hdc.is_null() && !self.old_bitmap.is_null() {
                let _ = SelectObject(self.hdc, self.old_bitmap);
            }
            if !self.bitmap.is_null() {
                let _ = DeleteObject(self.bitmap);
            }
            if !self.hdc.is_null() {
                let _ = DeleteDC(self.hdc);
            }
        }
    }
}

struct Dib {
    hdc: Hdc,
    bitmap: Hbitmap,
    old_bitmap: Hgdiobj,
    // Base returned by CreateDIBSection for the complete padded bitmap.
    storage_bits: *mut u8,
    // Logical (0, 0) exposed through Lock/GetSurfaceDesc. This address remains
    // stable for the lifetime of the Surface, including across Flip calls.
    bits: *mut u8,
    pitch: i32,
    storage_width: i32,
    storage_height: i32,
    guard_x: i32,
    guard_y: i32,
    info: Vec<u8>,
    saved_dc: i32,
}

unsafe impl Send for Dib {}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            if !self.hdc.is_null() && !self.old_bitmap.is_null() {
                let _ = SelectObject(self.hdc, self.old_bitmap);
            }
            if !self.bitmap.is_null() {
                let _ = DeleteObject(self.bitmap);
            }
            if !self.hdc.is_null() {
                let _ = DeleteDC(self.hdc);
            }
        }
    }
}

#[repr(C)]
pub struct Surface {
    vtable: *const usize,
    refs: AtomicU32,
    state: SharedState,
    width: Dword,
    height: Dword,
    bpp: Dword,
    caps: Dword,
    is_primary: bool,
    dib: Mutex<Dib>,
    attached: AtomicUsize,
    palette: AtomicUsize,
    clipper: AtomicUsize,
    color_key: Mutex<Option<DdColorKey>>,
}

unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

unsafe fn from_this(this: *mut c_void) -> *mut Surface {
    this.cast()
}

fn bytes_per_pixel(bpp: Dword) -> usize {
    match bpp {
        8 => 1,
        16 => 2,
        24 => 3,
        _ => 4,
    }
}

fn normalized_bpp(bpp: Dword) -> Dword {
    match bpp {
        8 | 16 | 24 | 32 => bpp,
        _ => 32,
    }
}

fn pixel_format(bpp: Dword) -> DdPixelFormat {
    let bpp = normalized_bpp(bpp);
    match bpp {
        8 => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB | DDPF_PALETTEINDEXED8,
            four_cc: 0,
            rgb_bit_count: 8,
            r_mask: 0,
            g_mask: 0,
            b_mask: 0,
            a_mask: 0,
        },
        16 => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB,
            four_cc: 0,
            rgb_bit_count: 16,
            r_mask: 0xF800,
            g_mask: 0x07E0,
            b_mask: 0x001F,
            a_mask: 0,
        },
        24 => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB,
            four_cc: 0,
            rgb_bit_count: 24,
            r_mask: 0x00FF_0000,
            g_mask: 0x0000_FF00,
            b_mask: 0x0000_00FF,
            a_mask: 0,
        },
        _ => DdPixelFormat {
            size: mem::size_of::<DdPixelFormat>() as u32,
            flags: DDPF_RGB,
            four_cc: 0,
            rgb_bit_count: 32,
            r_mask: 0x00FF_0000,
            g_mask: 0x0000_FF00,
            b_mask: 0x0000_00FF,
            a_mask: 0,
        },
    }
}

unsafe fn create_dib(width: Dword, height: Dword, bpp: Dword) -> Result<Dib, Hresult> {
    let bpp = normalized_bpp(bpp);
    let logical_width = width as usize;
    let logical_height = height as usize;
    if logical_width == 0
        || logical_height == 0
        || logical_width > MAX_SURFACE_DIMENSION
        || logical_height > MAX_SURFACE_DIMENSION
    {
        return Err(E_INVALIDARG);
    }

    let storage_width = logical_width
        .checked_add(GUARD_PIXELS * 2)
        .ok_or(E_INVALIDARG)?;
    let storage_height = logical_height
        .checked_add(GUARD_ROWS * 2)
        .ok_or(E_INVALIDARG)?;
    if storage_width > i32::MAX as usize || storage_height > i32::MAX as usize {
        return Err(E_INVALIDARG);
    }

    let row_bits = storage_width
        .checked_mul(bpp as usize)
        .ok_or(E_INVALIDARG)?;
    let pitch = row_bits
        .checked_add(31)
        .ok_or(E_INVALIDARG)?
        / 32
        * 4;
    let image_bytes = pitch
        .checked_mul(storage_height)
        .ok_or(E_INVALIDARG)?;
    if pitch > i32::MAX as usize || image_bytes > MAX_SURFACE_BYTES {
        return Err(E_INVALIDARG);
    }

    let color_entries = if bpp == 8 { 256 } else if bpp == 16 { 3 } else { 0 };
    let mut info = vec![0u8; mem::size_of::<BitmapInfoHeader>() + color_entries * 4];
    let header = info.as_mut_ptr().cast::<BitmapInfoHeader>();
    ptr::write(
        header,
        BitmapInfoHeader {
            bi_size: mem::size_of::<BitmapInfoHeader>() as u32,
            bi_width: storage_width as i32,
            bi_height: -(storage_height as i32),
            bi_planes: 1,
            bi_bit_count: bpp as u16,
            bi_compression: if bpp == 16 { BI_BITFIELDS } else { BI_RGB },
            bi_size_image: image_bytes.min(u32::MAX as usize) as u32,
            bi_x_pels_per_meter: 0,
            bi_y_pels_per_meter: 0,
            bi_clr_used: if bpp == 8 { 256 } else { 0 },
            bi_clr_important: 0,
        },
    );

    let table = info.as_mut_ptr().add(mem::size_of::<BitmapInfoHeader>());
    if bpp == 16 {
        let masks = table.cast::<u32>();
        *masks.add(0) = 0xF800;
        *masks.add(1) = 0x07E0;
        *masks.add(2) = 0x001F;
    } else if bpp == 8 {
        let colors = table.cast::<RgbQuad>();
        for index in 0..256usize {
            *colors.add(index) = RgbQuad {
                blue: index as u8,
                green: index as u8,
                red: index as u8,
                reserved: 0,
            };
        }
    }

    let hdc = CreateCompatibleDC(ptr::null_mut());
    if hdc.is_null() {
        return Err(E_FAIL);
    }
    let mut storage_bits: *mut c_void = ptr::null_mut();
    let bitmap = CreateDIBSection(
        ptr::null_mut(),
        info.as_ptr().cast(),
        DIB_RGB_COLORS,
        &mut storage_bits,
        ptr::null_mut(),
        0,
    );
    if bitmap.is_null() || storage_bits.is_null() {
        let _ = DeleteDC(hdc);
        return Err(E_FAIL);
    }
    let old_bitmap = SelectObject(hdc, bitmap.cast());
    if old_bitmap.is_null() || old_bitmap as isize == -1 {
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(hdc);
        return Err(E_FAIL);
    }

    let storage_bits = storage_bits.cast::<u8>();
    ptr::write_bytes(storage_bits, 0, image_bytes);
    let bytes = bytes_per_pixel(bpp);
    let logical_offset = GUARD_ROWS
        .checked_mul(pitch)
        .and_then(|value| value.checked_add(GUARD_PIXELS * bytes))
        .ok_or(E_INVALIDARG)?;
    let bits = storage_bits.add(logical_offset);

    Ok(Dib {
        hdc,
        bitmap,
        old_bitmap,
        storage_bits,
        bits,
        pitch: pitch as i32,
        storage_width: storage_width as i32,
        storage_height: storage_height as i32,
        guard_x: GUARD_PIXELS as i32,
        guard_y: GUARD_ROWS as i32,
        info,
        saved_dc: 0,
    })
}

unsafe fn normalize_dc(hdc: Hdc, viewport_x: i32, viewport_y: i32) {
    if hdc.is_null() {
        return;
    }
    let _ = SetGraphicsMode(hdc, GM_ADVANCED);
    let identity = Xform::default();
    let _ = SetWorldTransform(hdc, &identity);
    let _ = SetGraphicsMode(hdc, GM_COMPATIBLE);
    let _ = SetMapMode(hdc, MM_TEXT);
    let _ = SetWindowOrgEx(hdc, 0, 0, ptr::null_mut());
    let _ = SetViewportOrgEx(hdc, viewport_x, viewport_y, ptr::null_mut());
}

unsafe fn query_interface(
    this: *mut c_void,
    riid: *const Guid,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_IDIRECTDRAWSURFACE) {
        add_ref(this);
        *output = this;
        return S_OK;
    }
    E_NOINTERFACE
}

pub unsafe extern "system" fn add_ref(this: *mut c_void) -> Ulong {
    let surface = from_this(this);
    if surface.is_null() {
        return 0;
    }
    (*surface).refs.fetch_add(1, Ordering::Relaxed) + 1
}

pub unsafe extern "system" fn release(this: *mut c_void) -> Ulong {
    let surface = from_this(this);
    if surface.is_null() {
        return 0;
    }
    let remaining = (*surface).refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if remaining == 0 {
        if PRIMARY_SURFACE.load(Ordering::Acquire) == surface as usize {
            PRIMARY_SURFACE.store(0, Ordering::Release);
        }
        {
            let mut surfaces = ALL_SURFACES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            surfaces.retain(|pointer| *pointer != surface as usize);
        }
        let attached = (*surface).attached.swap(0, Ordering::AcqRel) as *mut c_void;
        if !attached.is_null() {
            let _ = release(attached);
        }
        let palette_ptr = (*surface).palette.swap(0, Ordering::AcqRel) as *mut c_void;
        if !palette_ptr.is_null() {
            let _ = palette::release(palette_ptr);
        }
        let clipper_ptr = (*surface).clipper.swap(0, Ordering::AcqRel) as *mut c_void;
        if !clipper_ptr.is_null() {
            let _ = clipper::release(clipper_ptr);
        }
        drop(Box::from_raw(surface));
    }
    remaining
}

unsafe fn full_rect(surface: *mut Surface) -> Rect {
    Rect {
        left: 0,
        top: 0,
        right: (*surface).width as i32,
        bottom: (*surface).height as i32,
    }
}

fn clamp_rect(rect: Rect, width: Dword, height: Dword) -> Rect {
    Rect {
        left: rect.left.clamp(0, width as i32),
        top: rect.top.clamp(0, height as i32),
        right: rect.right.clamp(0, width as i32),
        bottom: rect.bottom.clamp(0, height as i32),
    }
}


fn overlap_area(rect: Rect, width: Dword, height: Dword) -> i64 {
    let clipped = clamp_rect(rect, width, height);
    i64::from(clipped.width().max(0)) * i64::from(clipped.height().max(0))
}

unsafe fn primary_destination_rect(
    surface: *mut Surface,
    rect: Rect,
    operation: &str,
) -> Rect {
    if surface.is_null() || !(*surface).is_primary {
        return rect;
    }

    let (hwnd, fullscreen) = {
        let state = (*surface)
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (state.hwnd as Hwnd, state.fullscreen_requested)
    };
    if hwnd.is_null() || fullscreen {
        return rect;
    }

    let mut client_origin = Point::default();
    if ClientToScreen(hwnd, &mut client_origin) == FALSE {
        return rect;
    }

    let translated = Rect {
        left: rect.left.saturating_sub(client_origin.x),
        top: rect.top.saturating_sub(client_origin.y),
        right: rect.right.saturating_sub(client_origin.x),
        bottom: rect.bottom.saturating_sub(client_origin.y),
    };
    let original_overlap = overlap_area(rect, (*surface).width, (*surface).height);
    let translated_overlap = overlap_area(translated, (*surface).width, (*surface).height);

    // IDirectDraw windowed primary surfaces use desktop coordinates. Some
    // engines, however, already pass local coordinates when running through a
    // wrapper. Select the interpretation that covers more of the logical
    // primary surface, avoiding a compatibility regression for those callers.
    if translated_overlap > original_overlap {
        if PRIMARY_RECT_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) < 128 {
            log::line(&format!(
                "{} primary rect screen->local origin={},{} input={},{},{},{} output={},{},{},{} overlap={}=>{}",
                operation,
                client_origin.x,
                client_origin.y,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                translated.left,
                translated.top,
                translated.right,
                translated.bottom,
                original_overlap,
                translated_overlap
            ));
        }
        translated
    } else {
        rect
    }
}

unsafe fn read_native(pointer: *const u8, bytes: usize) -> u32 {
    match bytes {
        1 => *pointer as u32,
        2 => ptr::read_unaligned(pointer.cast::<u16>()) as u32,
        3 => *pointer as u32 | ((*pointer.add(1) as u32) << 8) | ((*pointer.add(2) as u32) << 16),
        _ => ptr::read_unaligned(pointer.cast::<u32>()),
    }
}

unsafe fn write_native(pointer: *mut u8, bytes: usize, value: u32) {
    match bytes {
        1 => *pointer = value as u8,
        2 => ptr::write_unaligned(pointer.cast::<u16>(), value as u16),
        3 => {
            *pointer = value as u8;
            *pointer.add(1) = (value >> 8) as u8;
            *pointer.add(2) = (value >> 16) as u8;
        }
        _ => ptr::write_unaligned(pointer.cast::<u32>(), value),
    }
}

unsafe fn copy_scaled(
    dst: *mut Surface,
    dst_rect: Rect,
    src: *mut Surface,
    src_rect: Rect,
    source_key: Option<DdColorKey>,
) -> Hresult {
    if dst.is_null() || src.is_null() {
        return E_POINTER;
    }
    if (*dst).bpp != (*src).bpp {
        return E_NOTIMPL;
    }

    let dst_width = dst_rect.width();
    let dst_height = dst_rect.height();
    let src_width = src_rect.width();
    let src_height = src_rect.height();
    if dst_width <= 0 || dst_height <= 0 || src_width <= 0 || src_height <= 0 {
        return DD_OK;
    }

    // Clip only the iteration area. Source sampling is still calculated from
    // the original, unclipped destination rectangle. The previous code
    // clamped destination and source independently, which rescaled a complete
    // 800x600 frame into the small residual rectangle left after an off-screen
    // window coordinate was clipped (for example roughly 430x440).
    let clipped_dst = clamp_rect(dst_rect, (*dst).width, (*dst).height);
    if clipped_dst.width() <= 0 || clipped_dst.height() <= 0 {
        return DD_OK;
    }

    let bytes = bytes_per_pixel((*dst).bpp);
    let snapshot = if dst == src {
        let dib = (*src).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let size = dib.pitch as usize * (*src).height as usize;
        let mut pixels = vec![0u8; size];
        ptr::copy_nonoverlapping(dib.bits, pixels.as_mut_ptr(), size);
        Some((pixels, dib.pitch))
    } else {
        None
    };

    let dst_dib = (*dst).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((pixels, source_pitch)) = snapshot.as_ref() {
        for y in clipped_dst.top..clipped_dst.bottom {
            let relative_y = i64::from(y - dst_rect.top);
            let sy = src_rect.top
                + ((relative_y * i64::from(src_height)) / i64::from(dst_height)) as i32;
            if sy < 0 || sy >= (*src).height as i32 {
                continue;
            }
            for x in clipped_dst.left..clipped_dst.right {
                let relative_x = i64::from(x - dst_rect.left);
                let sx = src_rect.left
                    + ((relative_x * i64::from(src_width)) / i64::from(dst_width)) as i32;
                if sx < 0 || sx >= (*src).width as i32 {
                    continue;
                }
                let source = pixels.as_ptr().add(
                    sy as usize * *source_pitch as usize + sx as usize * bytes,
                );
                let value = read_native(source, bytes);
                if source_key
                    .map(|key| value >= key.low && value <= key.high)
                    .unwrap_or(false)
                {
                    continue;
                }
                let target = dst_dib
                    .bits
                    .add(y as usize * dst_dib.pitch as usize + x as usize * bytes);
                ptr::copy_nonoverlapping(source, target, bytes);
            }
        }
        return DD_OK;
    }

    let src_dib = (*src).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for y in clipped_dst.top..clipped_dst.bottom {
        let relative_y = i64::from(y - dst_rect.top);
        let sy = src_rect.top
            + ((relative_y * i64::from(src_height)) / i64::from(dst_height)) as i32;
        if sy < 0 || sy >= (*src).height as i32 {
            continue;
        }
        for x in clipped_dst.left..clipped_dst.right {
            let relative_x = i64::from(x - dst_rect.left);
            let sx = src_rect.left
                + ((relative_x * i64::from(src_width)) / i64::from(dst_width)) as i32;
            if sx < 0 || sx >= (*src).width as i32 {
                continue;
            }
            let source = src_dib
                .bits
                .add(sy as usize * src_dib.pitch as usize + sx as usize * bytes);
            let value = read_native(source, bytes);
            if source_key
                .map(|key| value >= key.low && value <= key.high)
                .unwrap_or(false)
            {
                continue;
            }
            let target = dst_dib
                .bits
                .add(y as usize * dst_dib.pitch as usize + x as usize * bytes);
            ptr::copy_nonoverlapping(source, target, bytes);
        }
    }
    DD_OK
}

unsafe fn fill_native(surface: *mut Surface, rect: Rect, value: Dword) -> Hresult {
    let rect = clamp_rect(rect, (*surface).width, (*surface).height);
    if rect.width() <= 0 || rect.height() <= 0 {
        return DD_OK;
    }
    let bytes = bytes_per_pixel((*surface).bpp);
    let dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for y in rect.top as usize..rect.bottom as usize {
        for x in rect.left as usize..rect.right as usize {
            let target = dib.bits.add(y * dib.pitch as usize + x * bytes);
            write_native(target, bytes, value);
        }
    }
    DD_OK
}

pub unsafe fn present_to_dc(surface: *mut Surface, hwnd: Hwnd, dst: Hdc) {
    if surface.is_null() || hwnd.is_null() || dst.is_null() {
        return;
    }

    let mut client = Rect::default();
    if GetClientRect(hwnd, &mut client) == FALSE || client.width() <= 0 || client.height() <= 0 {
        return;
    }

    let view = window::viewport(
        client.width(),
        client.height(),
        (*surface).width as i32,
        (*surface).height as i32,
    );

    let mut presenter = PRESENTER_BUFFER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let recreate = presenter
        .as_ref()
        .map(|buffer| {
            buffer.hwnd != hwnd as usize
                || buffer.width != client.width()
                || buffer.height != client.height()
        })
        .unwrap_or(true);

    if recreate {
        *presenter = None;
        let memory_dc = CreateCompatibleDC(dst);
        if memory_dc.is_null() {
            return;
        }
        let bitmap = CreateCompatibleBitmap(dst, client.width(), client.height());
        if bitmap.is_null() {
            let _ = DeleteDC(memory_dc);
            return;
        }
        let old_bitmap = SelectObject(memory_dc, bitmap);
        if old_bitmap.is_null() {
            let _ = DeleteObject(bitmap);
            let _ = DeleteDC(memory_dc);
            return;
        }
        *presenter = Some(PresenterBuffer {
            hwnd: hwnd as usize,
            width: client.width(),
            height: client.height(),
            hdc: memory_dc,
            bitmap,
            old_bitmap,
        });
        log::line(&format!(
            "presenter buffer hwnd=0x{:08X} client={}x{} source={}x{} viewport={},{} {}x{}",
            hwnd as usize,
            client.width(),
            client.height(),
            (*surface).width,
            (*surface).height,
            view.left,
            view.top,
            view.width(),
            view.height()
        ));
    }

    let Some(buffer) = presenter.as_mut() else {
        return;
    };

    // Compose the complete frame off-screen. The old implementation cleared
    // the real window DC to black before a slow HALFTONE stretch operation, which
    // exposed a black intermediate frame and caused severe flicker.
    let _ = PatBlt(buffer.hdc, 0, 0, buffer.width, buffer.height, BLACKNESS);

    #[cfg(feature = "nearest-neighbor")]
    let stretch_mode = COLORONCOLOR;
    #[cfg(not(feature = "nearest-neighbor"))]
    let stretch_mode = HALFTONE;

    let previous_mode = SetStretchBltMode(buffer.hdc, stretch_mode);
    if stretch_mode == HALFTONE {
        let _ = SetBrushOrgEx(buffer.hdc, 0, 0, ptr::null_mut());
    }

    {
        let dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let copied = StretchDIBits(
            buffer.hdc,
            view.left,
            view.top,
            view.width(),
            view.height(),
            dib.guard_x,
            dib.guard_y,
            (*surface).width as i32,
            (*surface).height as i32,
            dib.storage_bits.cast(),
            dib.info.as_ptr().cast(),
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        if copied == 0 {
            log::line("StretchDIBits failed while presenting primary surface");
        }
    }

    if previous_mode != 0 {
        let _ = SetStretchBltMode(buffer.hdc, previous_mode);
    }

    // Publish with a clean MM_TEXT destination DC. Legacy engines often use
    // CS_OWNDC and leave anisotropic viewport state behind; applying our
    // client-size coordinates through that state is what produced the
    // approximately 430x440 image inside an 800x600 client.
    let saved_dst = SaveDC(dst);
    if saved_dst != 0 {
        normalize_dc(dst, 0, 0);
    }
    let _ = BitBlt(
        dst,
        0,
        0,
        buffer.width,
        buffer.height,
        buffer.hdc,
        0,
        0,
        SRCCOPY,
    );
    if saved_dst != 0 {
        let _ = RestoreDC(dst, saved_dst);
    }
}

pub unsafe fn present(surface: *mut Surface) {
    if surface.is_null() {
        return;
    }
    let state = (*surface).state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let hwnd = state.hwnd as Hwnd;
    drop(state);
    if hwnd.is_null() {
        return;
    }
    let dst = GetDC(hwnd);
    if dst.is_null() {
        return;
    }
    present_to_dc(surface, hwnd, dst);
    let _ = ReleaseDC(hwnd, dst);
}

pub unsafe fn present_global_primary_to_dc(hwnd: Hwnd, dst: Hdc) {
    let pointer = PRIMARY_SURFACE.load(Ordering::Acquire) as *mut Surface;
    if !pointer.is_null() {
        present_to_dc(pointer, hwnd, dst);
    }
}

pub fn release_presenter(hwnd: Hwnd) {
    let mut presenter = PRESENTER_BUFFER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if presenter
        .as_ref()
        .map(|buffer| buffer.hwnd == hwnd as usize)
        .unwrap_or(false)
    {
        *presenter = None;
    }
}

unsafe extern "system" fn add_attached_surface(this: *mut c_void, attached: *mut c_void) -> Hresult {
    if attached.is_null() {
        return E_POINTER;
    }
    let surface = from_this(this);
    let old = (*surface).attached.swap(attached as usize, Ordering::AcqRel) as *mut c_void;
    add_ref(attached);
    if !old.is_null() {
        let _ = release(old);
    }
    DD_OK
}

unsafe extern "system" fn add_overlay_dirty_rect(_this: *mut c_void, _rect: *mut Rect) -> Hresult {
    DD_OK
}

unsafe extern "system" fn blt(
    this: *mut c_void,
    dst_rect: *mut Rect,
    source: *mut c_void,
    src_rect: *mut Rect,
    flags: Dword,
    effects: *mut DdBltFxPrefix,
) -> Hresult {
    let dst = from_this(this);
    let target_rect = if dst_rect.is_null() {
        full_rect(dst)
    } else {
        primary_destination_rect(dst, *dst_rect, "Blt")
    };
    let hr = if flags & DDBLT_COLORFILL != 0 {
        if effects.is_null() {
            E_INVALIDARG
        } else {
            fill_native(dst, target_rect, (*effects).fill_color)
        }
    } else if source.is_null() {
        E_POINTER
    } else {
        let src = from_this(source);
        let source_rect = if src_rect.is_null() { full_rect(src) } else { *src_rect };
        let source_key = if flags & DDBLT_KEYSRCOVERRIDE != 0 && !effects.is_null() {
            Some((*effects).src_color_key)
        } else if flags & DDBLT_KEYSRC != 0 {
            *(*src).color_key.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
        } else {
            None
        };
        copy_scaled(dst, target_rect, src, source_rect, source_key)
    };
    if hr == DD_OK && (*dst).is_primary {
        present(dst);
    }
    hr
}

unsafe extern "system" fn blt_batch(
    _this: *mut c_void,
    _batch: *mut c_void,
    _count: Dword,
    _flags: Dword,
) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn blt_fast(
    this: *mut c_void,
    x: Dword,
    y: Dword,
    source: *mut c_void,
    src_rect: *mut Rect,
    flags: Dword,
) -> Hresult {
    if source.is_null() {
        return E_POINTER;
    }
    let dst = from_this(this);
    let src = from_this(source);
    let source_rect = if src_rect.is_null() { full_rect(src) } else { *src_rect };
    let target_rect = primary_destination_rect(
        dst,
        Rect {
            left: x as i32,
            top: y as i32,
            right: x as i32 + source_rect.width(),
            bottom: y as i32 + source_rect.height(),
        },
        "BltFast",
    );
    let source_key = if flags & DDBLTFAST_SRCCOLORKEY != 0 {
        *(*src).color_key.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    } else {
        None
    };
    let hr = copy_scaled(dst, target_rect, src, source_rect, source_key);
    if hr == DD_OK && (*dst).is_primary {
        present(dst);
    }
    hr
}

unsafe extern "system" fn delete_attached_surface(
    this: *mut c_void,
    _flags: Dword,
    attached: *mut c_void,
) -> Hresult {
    let surface = from_this(this);
    let current = (*surface).attached.load(Ordering::Acquire) as *mut c_void;
    if current == attached && !current.is_null() {
        (*surface).attached.store(0, Ordering::Release);
        let _ = release(current);
    }
    DD_OK
}

unsafe extern "system" fn enum_attached_surfaces(
    this: *mut c_void,
    context: *mut c_void,
    callback: *mut c_void,
) -> Hresult {
    if callback.is_null() {
        return E_POINTER;
    }
    let surface = from_this(this);
    let attached = (*surface).attached.load(Ordering::Acquire) as *mut c_void;
    if attached.is_null() {
        return DD_OK;
    }
    let function: unsafe extern "system" fn(*mut c_void, *mut DdSurfaceDesc, *mut c_void) -> Hresult = mem::transmute(callback);
    let mut desc = DdSurfaceDesc::default();
    let _ = get_surface_desc(attached, &mut desc);
    let _ = function(attached, &mut desc, context);
    DD_OK
}

unsafe extern "system" fn enum_overlay_z_orders(
    _this: *mut c_void,
    _flags: Dword,
    _context: *mut c_void,
    _callback: *mut c_void,
) -> Hresult {
    DD_OK
}

unsafe extern "system" fn flip(
    this: *mut c_void,
    target_override: *mut c_void,
    _flags: Dword,
) -> Hresult {
    let primary = from_this(this);
    let back = if !target_override.is_null() {
        from_this(target_override)
    } else {
        (*primary).attached.load(Ordering::Acquire) as *mut Surface
    };
    if back.is_null() || back == primary {
        present(primary);
        return DD_OK;
    }
    if (*primary).width != (*back).width || (*primary).height != (*back).height || (*primary).bpp != (*back).bpp {
        return E_INVALIDARG;
    }
    let front_dib = (*primary).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let back_dib = (*back).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if front_dib.pitch != back_dib.pitch
        || front_dib.storage_width != back_dib.storage_width
        || front_dib.storage_height != back_dib.storage_height
    {
        return E_INVALIDARG;
    }

    // Never exchange Dib/HDC objects. GetSurfaceDesc and Lock expose pointers
    // that legacy engines may cache beyond one call. Swapping the backing Dib
    // changed those addresses after every Flip and also paired ReleaseDC with
    // the wrong HDC. Exchange only the pixel contents so object identity,
    // lpSurface and HDC stay stable.
    let byte_count = front_dib.pitch as usize * front_dib.storage_height as usize;
    ptr::swap_nonoverlapping(front_dib.storage_bits, back_dib.storage_bits, byte_count);
    drop(back_dib);
    drop(front_dib);
    present(primary);
    DD_OK
}

unsafe extern "system" fn get_attached_surface(
    this: *mut c_void,
    _caps: *mut DdCaps,
    output: *mut *mut c_void,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let surface = from_this(this);
    let attached = (*surface).attached.load(Ordering::Acquire) as *mut c_void;
    if attached.is_null() {
        return E_FAIL;
    }
    add_ref(attached);
    *output = attached;
    DD_OK
}

unsafe extern "system" fn get_blt_status(_this: *mut c_void, _flags: Dword) -> Hresult {
    DD_OK
}

unsafe extern "system" fn get_caps(this: *mut c_void, caps: *mut DdCaps) -> Hresult {
    if caps.is_null() {
        return E_POINTER;
    }
    (*caps).caps = (*from_this(this)).caps;
    DD_OK
}

unsafe extern "system" fn get_clipper(this: *mut c_void, output: *mut *mut c_void) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let pointer = (*from_this(this)).clipper.load(Ordering::Acquire) as *mut c_void;
    if pointer.is_null() {
        return E_FAIL;
    }
    clipper::add_ref(pointer);
    *output = pointer;
    DD_OK
}

unsafe extern "system" fn get_color_key(
    this: *mut c_void,
    _flags: Dword,
    output: *mut DdColorKey,
) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    let value = *(*from_this(this)).color_key.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match value {
        Some(key) => {
            *output = key;
            DD_OK
        }
        None => E_FAIL,
    }
}

unsafe extern "system" fn get_dc(this: *mut c_void, output: *mut Hdc) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    let surface = from_this(this);
    let mut dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if dib.saved_dc == 0 {
        dib.saved_dc = SaveDC(dib.hdc);
        normalize_dc(dib.hdc, dib.guard_x, dib.guard_y);
    }
    *output = dib.hdc;
    DD_OK
}

unsafe extern "system" fn get_flip_status(_this: *mut c_void, _flags: Dword) -> Hresult {
    DD_OK
}

unsafe extern "system" fn get_overlay_position(
    _this: *mut c_void,
    x: *mut Long,
    y: *mut Long,
) -> Hresult {
    if x.is_null() || y.is_null() {
        return E_POINTER;
    }
    *x = 0;
    *y = 0;
    DD_OK
}

unsafe extern "system" fn get_palette(this: *mut c_void, output: *mut *mut c_void) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = ptr::null_mut();
    let pointer = (*from_this(this)).palette.load(Ordering::Acquire) as *mut c_void;
    if pointer.is_null() {
        return E_FAIL;
    }
    palette::add_ref(pointer);
    *output = pointer;
    DD_OK
}

unsafe extern "system" fn get_pixel_format(this: *mut c_void, output: *mut DdPixelFormat) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    *output = pixel_format((*from_this(this)).bpp);
    DD_OK
}

unsafe fn fill_desc(surface: *mut Surface, output: *mut DdSurfaceDesc) -> Hresult {
    if output.is_null() {
        return E_POINTER;
    }
    let dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *output = DdSurfaceDesc {
        size: mem::size_of::<DdSurfaceDesc>() as u32,
        flags: DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_LPSURFACE | DDSD_PIXELFORMAT,
        height: (*surface).height,
        width: (*surface).width,
        pitch: dib.pitch,
        back_buffer_count: if (*surface).attached.load(Ordering::Acquire) != 0 { 1 } else { 0 },
        mip_map_count: 0,
        alpha_bit_depth: 0,
        reserved: 0,
        surface: dib.bits.cast(),
        ck_dest_overlay: DdColorKey::default(),
        ck_dest_blt: DdColorKey::default(),
        ck_src_overlay: DdColorKey::default(),
        ck_src_blt: (*surface).color_key.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).unwrap_or_default(),
        pixel_format: pixel_format((*surface).bpp),
        caps: DdCaps { caps: (*surface).caps },
    };
    DD_OK
}

unsafe extern "system" fn get_surface_desc(this: *mut c_void, output: *mut DdSurfaceDesc) -> Hresult {
    fill_desc(from_this(this), output)
}

unsafe extern "system" fn initialize(
    _this: *mut c_void,
    _ddraw: *mut c_void,
    _desc: *mut DdSurfaceDesc,
) -> Hresult {
    DD_OK
}

unsafe extern "system" fn is_lost(_this: *mut c_void) -> Hresult {
    DD_OK
}

unsafe extern "system" fn lock(
    this: *mut c_void,
    rect: *mut Rect,
    output: *mut DdSurfaceDesc,
    _flags: Dword,
    _event: Handle,
) -> Hresult {
    let surface = from_this(this);
    let hr = fill_desc(surface, output);
    if hr != DD_OK || rect.is_null() {
        return hr;
    }

    let locked = clamp_rect(*rect, (*surface).width, (*surface).height);
    if locked.width() <= 0 || locked.height() <= 0 {
        return E_INVALIDARG;
    }
    let bytes = bytes_per_pixel((*surface).bpp);
    let dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    (*output).surface = dib
        .bits
        .add(locked.top as usize * dib.pitch as usize + locked.left as usize * bytes)
        .cast();
    (*output).width = locked.width() as u32;
    (*output).height = locked.height() as u32;
    DD_OK
}

unsafe extern "system" fn release_dc(this: *mut c_void, _hdc: Hdc) -> Hresult {
    let surface = from_this(this);
    {
        let mut dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if DC_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) < 16 {
            let mut window_org = Point::default();
            let mut viewport_org = Point::default();
            let mut window_ext = Point::default();
            let mut viewport_ext = Point::default();
            let map_mode = GetMapMode(dib.hdc);
            let _ = GetWindowOrgEx(dib.hdc, &mut window_org);
            let _ = GetViewportOrgEx(dib.hdc, &mut viewport_org);
            let _ = GetWindowExtEx(dib.hdc, &mut window_ext);
            let _ = GetViewportExtEx(dib.hdc, &mut viewport_ext);
            log::line(&format!(
                "surface ReleaseDC ptr=0x{:08X} {}x{} primary={} map={} window_org={},{} viewport_org={},{} window_ext={}x{} viewport_ext={}x{}",
                surface as usize,
                (*surface).width,
                (*surface).height,
                (*surface).is_primary,
                map_mode,
                window_org.x,
                window_org.y,
                viewport_org.x,
                viewport_org.y,
                window_ext.x,
                window_ext.y,
                viewport_ext.x,
                viewport_ext.y
            ));
        }
        if dib.saved_dc != 0 {
            let saved = dib.saved_dc;
            dib.saved_dc = 0;
            let _ = RestoreDC(dib.hdc, saved);
        } else {
            normalize_dc(dib.hdc, dib.guard_x, dib.guard_y);
        }
    }
    if (*surface).is_primary {
        present(surface);
    }
    DD_OK
}

unsafe extern "system" fn restore(_this: *mut c_void) -> Hresult {
    DD_OK
}

unsafe extern "system" fn set_clipper(this: *mut c_void, value: *mut c_void) -> Hresult {
    let surface = from_this(this);
    if !value.is_null() {
        clipper::add_ref(value);
    }
    let old = (*surface).clipper.swap(value as usize, Ordering::AcqRel) as *mut c_void;
    if !old.is_null() {
        let _ = clipper::release(old);
    }
    DD_OK
}

unsafe extern "system" fn set_color_key(
    this: *mut c_void,
    _flags: Dword,
    value: *mut DdColorKey,
) -> Hresult {
    let surface = from_this(this);
    let mut key = (*surface).color_key.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *key = if value.is_null() { None } else { Some(*value) };
    DD_OK
}

unsafe extern "system" fn set_overlay_position(
    _this: *mut c_void,
    _x: Long,
    _y: Long,
) -> Hresult {
    DD_OK
}

unsafe fn apply_palette(surface: *mut Surface) {
    if surface.is_null() || (*surface).bpp != 8 {
        return;
    }
    let palette_ptr = (*surface).palette.load(Ordering::Acquire) as *mut c_void;
    if palette_ptr.is_null() {
        return;
    }
    let Some(entries) = palette::snapshot(palette_ptr) else {
        return;
    };
    let mut colors = [RgbQuad::default(); 256];
    for (index, entry) in entries.iter().enumerate() {
        colors[index] = RgbQuad {
            blue: entry.blue,
            green: entry.green,
            red: entry.red,
            reserved: 0,
        };
    }
    let mut dib = (*surface).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = SetDIBColorTable(dib.hdc, 0, 256, colors.as_ptr());
    let table = dib.info.as_mut_ptr().add(mem::size_of::<BitmapInfoHeader>()).cast::<RgbQuad>();
    ptr::copy_nonoverlapping(colors.as_ptr(), table, colors.len());
}

unsafe extern "system" fn set_palette(this: *mut c_void, value: *mut c_void) -> Hresult {
    let surface = from_this(this);
    if !value.is_null() {
        palette::add_ref(value);
    }
    let old = (*surface).palette.swap(value as usize, Ordering::AcqRel) as *mut c_void;
    if !old.is_null() {
        let _ = palette::release(old);
    }
    apply_palette(surface);
    let attached = (*surface).attached.load(Ordering::Acquire) as *mut Surface;
    if !attached.is_null() {
        if !value.is_null() {
            palette::add_ref(value);
        }
        let attached_old = (*attached).palette.swap(value as usize, Ordering::AcqRel) as *mut c_void;
        if !attached_old.is_null() {
            let _ = palette::release(attached_old);
        }
        apply_palette(attached);
    }
    DD_OK
}

unsafe extern "system" fn unlock(this: *mut c_void, _surface_data: *mut c_void) -> Hresult {
    let surface = from_this(this);
    if (*surface).is_primary {
        present(surface);
    }
    DD_OK
}

unsafe extern "system" fn update_overlay(
    _this: *mut c_void,
    _src_rect: *mut Rect,
    _dst: *mut c_void,
    _dst_rect: *mut Rect,
    _flags: Dword,
    _effects: *mut c_void,
) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn update_overlay_display(_this: *mut c_void, _flags: Dword) -> Hresult {
    E_NOTIMPL
}

unsafe extern "system" fn update_overlay_z_order(
    _this: *mut c_void,
    _flags: Dword,
    _reference: *mut c_void,
) -> Hresult {
    E_NOTIMPL
}

fn vtable() -> *const usize {
    static TABLE: OnceLock<Box<[usize; VTABLE_LEN]>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            Box::new([
                query_interface as *const () as usize,
                add_ref as *const () as usize,
                release as *const () as usize,
                add_attached_surface as *const () as usize,
                add_overlay_dirty_rect as *const () as usize,
                blt as *const () as usize,
                blt_batch as *const () as usize,
                blt_fast as *const () as usize,
                delete_attached_surface as *const () as usize,
                enum_attached_surfaces as *const () as usize,
                enum_overlay_z_orders as *const () as usize,
                flip as *const () as usize,
                get_attached_surface as *const () as usize,
                get_blt_status as *const () as usize,
                get_caps as *const () as usize,
                get_clipper as *const () as usize,
                get_color_key as *const () as usize,
                get_dc as *const () as usize,
                get_flip_status as *const () as usize,
                get_overlay_position as *const () as usize,
                get_palette as *const () as usize,
                get_pixel_format as *const () as usize,
                get_surface_desc as *const () as usize,
                initialize as *const () as usize,
                is_lost as *const () as usize,
                lock as *const () as usize,
                release_dc as *const () as usize,
                restore as *const () as usize,
                set_clipper as *const () as usize,
                set_color_key as *const () as usize,
                set_overlay_position as *const () as usize,
                set_palette as *const () as usize,
                unlock as *const () as usize,
                update_overlay as *const () as usize,
                update_overlay_display as *const () as usize,
                update_overlay_z_order as *const () as usize,
            ])
        })
        .as_ptr()
}

pub unsafe fn create(
    state: SharedState,
    width: Dword,
    height: Dword,
    bpp: Dword,
    caps: Dword,
    is_primary: bool,
) -> Result<*mut c_void, Hresult> {
    let bpp = normalized_bpp(bpp);
    let dib = create_dib(width, height, bpp)?;
    let object = Box::new(Surface {
        vtable: vtable(),
        refs: AtomicU32::new(1),
        state,
        width,
        height,
        bpp,
        caps,
        is_primary,
        dib: Mutex::new(dib),
        attached: AtomicUsize::new(0),
        palette: AtomicUsize::new(0),
        clipper: AtomicUsize::new(0),
        color_key: Mutex::new(None),
    });
    let pointer = Box::into_raw(object);
    {
        let mut surfaces = ALL_SURFACES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        surfaces.push(pointer as usize);
    }
    if is_primary {
        PRIMARY_SURFACE.store(pointer as usize, Ordering::Release);
    }
    {
        let dib = (*pointer).dib.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        log::trace(&format!(
            "surface create ptr=0x{:08X} logical={}x{}x{} pitch={} storage={}x{} guard={}x{} caps=0x{:08X} primary={}",
            pointer as usize,
            width,
            height,
            bpp,
            dib.pitch,
            dib.storage_width,
            dib.storage_height,
            dib.guard_x,
            dib.guard_y,
            caps,
            is_primary
        ));
    }
    Ok(pointer.cast())
}

pub unsafe fn attach(primary: *mut c_void, back: *mut c_void) {
    if primary.is_null() || back.is_null() {
        return;
    }
    let primary = from_this(primary);
    (*primary).attached.store(back as usize, Ordering::Release);
}

pub unsafe fn duplicate(source: *mut c_void) -> Result<*mut c_void, Hresult> {
    if source.is_null() {
        return Err(E_POINTER);
    }
    let source_surface = from_this(source);
    let duplicate = create(
        (*source_surface).state.clone(),
        (*source_surface).width,
        (*source_surface).height,
        (*source_surface).bpp,
        (*source_surface).caps,
        false,
    )?;
    let duplicate_surface = from_this(duplicate);
    let _ = copy_scaled(
        duplicate_surface,
        full_rect(duplicate_surface),
        source_surface,
        full_rect(source_surface),
        None,
    );
    Ok(duplicate)
}

pub unsafe fn palette_changed(palette_pointer: *mut c_void) {
    let surfaces = {
        let guard = ALL_SURFACES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    };
    for pointer in surfaces {
        let surface = pointer as *mut Surface;
        if !surface.is_null() && (*surface).palette.load(Ordering::Acquire) == palette_pointer as usize {
            apply_palette(surface);
        }
    }
}

pub fn primary_pointer() -> *mut c_void {
    PRIMARY_SURFACE.load(Ordering::Acquire) as *mut c_void
}
