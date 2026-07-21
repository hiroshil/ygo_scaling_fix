use crate::abi::{Dword, Long, Rect};
use std::sync::{Arc, Mutex};

pub type SharedState = Arc<Mutex<DrawState>>;

#[derive(Debug)]
pub struct DrawState {
    pub hwnd: usize,
    pub logical_width: Dword,
    pub logical_height: Dword,
    pub bpp: Dword,
    pub cooperative_flags: Dword,
    pub fullscreen_requested: bool,
    pub original_style: Long,
    pub original_ex_style: Long,
    pub original_rect: Rect,
    pub saved_window_state: bool,
}

impl Default for DrawState {
    fn default() -> Self {
        Self {
            hwnd: 0,
            logical_width: 800,
            logical_height: 600,
            bpp: 16,
            cooperative_flags: 0,
            fullscreen_requested: false,
            original_style: 0,
            original_ex_style: 0,
            original_rect: Rect::default(),
            saved_window_state: false,
        }
    }
}

pub fn new_shared() -> SharedState {
    Arc::new(Mutex::new(DrawState::default()))
}
