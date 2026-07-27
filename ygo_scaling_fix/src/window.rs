use crate::abi::*;
use crate::log;
use crate::state::SharedState;
use crate::surface;
use core::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Weak};

static ACTIVE_STATE: Mutex<Option<Weak<Mutex<crate::state::DrawState>>>> = Mutex::new(None);
static PREVIOUS_WNDPROC: AtomicUsize = AtomicUsize::new(0);
static HOOKED_HWND: AtomicUsize = AtomicUsize::new(0);

pub fn viewport(
    client_width: i32,
    client_height: i32,
    logical_width: i32,
    logical_height: i32,
) -> Rect {
    if client_width <= 0 || client_height <= 0 || logical_width <= 0 || logical_height <= 0 {
        return Rect {
            left: 0,
            top: 0,
            right: client_width.max(0),
            bottom: client_height.max(0),
        };
    }

    #[cfg(feature = "aspect-4x3")]
    {
        let scale_x = client_width as f64 / logical_width as f64;
        let scale_y = client_height as f64 / logical_height as f64;
        let scale = scale_x.min(scale_y);
        let width = (logical_width as f64 * scale).round() as i32;
        let height = (logical_height as f64 * scale).round() as i32;
        let x = (client_width - width) / 2;
        let y = (client_height - height) / 2;
        return Rect {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };
    }

    #[cfg(not(feature = "aspect-4x3"))]
    {
        Rect {
            left: 0,
            top: 0,
            right: client_width,
            bottom: client_height,
        }
    }
}

fn unpack_mouse(lparam: Lparam) -> (i32, i32) {
    let raw = lparam as u32;
    let x = (raw as u16) as i16 as i32;
    let y = ((raw >> 16) as u16) as i16 as i32;
    (x, y)
}

fn pack_mouse(x: i32, y: i32) -> Lparam {
    let low = x.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16 as u32;
    let high = y.clamp(i16::MIN as i32, i16::MAX as i32) as i16 as u16 as u32;
    (low | (high << 16)) as isize
}

fn remap_mouse(hwnd: Hwnd, lparam: Lparam) -> Lparam {
    let state = {
        let guard = ACTIVE_STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.as_ref().and_then(Weak::upgrade)
    };
    let Some(state) = state else {
        return lparam;
    };
    let state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut client = Rect::default();
    if unsafe { GetClientRect(hwnd, &mut client) } == FALSE {
        return lparam;
    }
    let view = viewport(
        client.width(),
        client.height(),
        state.logical_width as i32,
        state.logical_height as i32,
    );
    if view.width() <= 0 || view.height() <= 0 {
        return lparam;
    }
    let (x, y) = unpack_mouse(lparam);
    let logical_x = ((x - view.left) as i64 * state.logical_width as i64 / view.width() as i64)
        .clamp(0, state.logical_width.saturating_sub(1) as i64) as i32;
    let logical_y = ((y - view.top) as i64 * state.logical_height as i64 / view.height() as i64)
        .clamp(0, state.logical_height.saturating_sub(1) as i64) as i32;
    pack_mouse(logical_x, logical_y)
}

unsafe extern "system" fn wndproc(
    hwnd: Hwnd,
    message: Uint,
    wparam: Wparam,
    mut lparam: Lparam,
) -> Lresult {
    if message == WM_ERASEBKGND {
        // The complete client area is supplied by the presenter. Reporting the
        // erase as handled prevents the class brush from flashing underneath.
        return 1;
    }

    if message == WM_PAINT {
        let mut paint = PaintStruct::default();
        let hdc = BeginPaint(hwnd, &mut paint);
        if !hdc.is_null() {
            surface::present_global_primary_to_dc(hwnd, hdc);
        }
        let _ = EndPaint(hwnd, &paint);
        return 0;
    }

    if mouse_message(message) {
        lparam = remap_mouse(hwnd, lparam);
    }

    let previous = PREVIOUS_WNDPROC.load(Ordering::Acquire);
    let result = if previous != 0 {
        CallWindowProcA(previous as *const c_void, hwnd, message, wparam, lparam)
    } else {
        DefWindowProcA(hwnd, message, wparam, lparam)
    };


    if message == WM_NCDESTROY {
        surface::release_presenter(hwnd);
        if previous != 0 {
            let _ = SetWindowLongA(hwnd, GWL_WNDPROC, previous as u32 as i32);
        }
        PREVIOUS_WNDPROC.store(0, Ordering::Release);
        HOOKED_HWND.store(0, Ordering::Release);
        let mut guard = ACTIVE_STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = None;
    }

    result
}

pub unsafe fn install(hwnd: Hwnd, state: &SharedState) {
    if hwnd.is_null() {
        return;
    }

    {
        let mut guard = ACTIVE_STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(std::sync::Arc::downgrade(state));
    }

    let hwnd_value = hwnd as usize;
    if HOOKED_HWND.load(Ordering::Acquire) == hwnd_value {
        return;
    }

    let current = GetWindowLongA(hwnd, GWL_WNDPROC) as u32 as usize;
    let relay = wndproc as *const () as usize;
    if current == relay {
        HOOKED_HWND.store(hwnd_value, Ordering::Release);
        return;
    }

    let previous = SetWindowLongA(hwnd, GWL_WNDPROC, relay as u32 as i32) as u32 as usize;
    PREVIOUS_WNDPROC.store(previous, Ordering::Release);
    HOOKED_HWND.store(hwnd_value, Ordering::Release);
    log::line(&format!(
        "window relay installed hwnd=0x{hwnd_value:08X} previous=0x{previous:08X}"
    ));
}

pub unsafe fn enter_borderless(state: &SharedState) {
    let (hwnd, logical_width, logical_height) = {
        let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let hwnd = state.hwnd as Hwnd;
        if hwnd.is_null() {
            return;
        }

        if !state.saved_window_state {
            state.original_style = GetWindowLongA(hwnd, GWL_STYLE);
            state.original_ex_style = GetWindowLongA(hwnd, GWL_EXSTYLE);
            let _ = GetWindowRect(hwnd, &mut state.original_rect);
            state.saved_window_state = true;
        }

        (hwnd, state.logical_width, state.logical_height)
    };

    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut info = MonitorInfoA {
        cb_size: core::mem::size_of::<MonitorInfoA>() as u32,
        ..MonitorInfoA::default()
    };
    if monitor.is_null() || GetMonitorInfoA(monitor, &mut info) == FALSE {
        return;
    }

    let _ = SetWindowLongA(hwnd, GWL_STYLE, WS_POPUP | WS_VISIBLE);
    let _ = SetWindowLongA(hwnd, GWL_EXSTYLE, 0);
    let rect = info.rc_monitor;
    let _ = SetWindowPos(
        hwnd,
        ptr::null_mut(),
        rect.left,
        rect.top,
        rect.width(),
        rect.height(),
        SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
    );
    log::line(&format!(
        "borderless native monitor {}x{} logical={}x{}",
        rect.width(),
        rect.height(),
        logical_width,
        logical_height
    ));
}

pub unsafe fn leave_borderless(state: &SharedState) {
    let snapshot = {
        let mut state = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let hwnd = state.hwnd as Hwnd;
        if hwnd.is_null() || !state.saved_window_state {
            return;
        }
        let snapshot = (
            hwnd,
            state.original_style,
            state.original_ex_style,
            state.original_rect,
        );
        state.saved_window_state = false;
        snapshot
    };

    let (hwnd, style, ex_style, rect) = snapshot;
    surface::release_presenter(hwnd);
    let _ = SetWindowLongA(hwnd, GWL_STYLE, style);
    let _ = SetWindowLongA(hwnd, GWL_EXSTYLE, ex_style);
    let ok = SetWindowPos(
        hwnd,
        ptr::null_mut(),
        rect.left,
        rect.top,
        rect.width().max(1),
        rect.height().max(1),
        SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
    );

    let mut client = Rect::default();
    let client_size = if GetClientRect(hwnd, &mut client) != FALSE {
        (client.width(), client.height())
    } else {
        (0, 0)
    };
    log::line(&format!(
        "restored exact windowed rectangle outer={}x{} client={}x{} at {},{} ok={}",
        rect.width(),
        rect.height(),
        client_size.0,
        client_size.1,
        rect.left,
        rect.top,
        ok
    ));
}
