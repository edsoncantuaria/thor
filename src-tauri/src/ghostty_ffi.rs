//! FFI para o shim C do libghostty (`ghostty_shim.m`).
//!

//!

#![cfg(all(target_os = "macos", ghostty_linked))]

use std::os::raw::{c_char, c_void};

pub type ThorSurface = *mut c_void;

#[allow(dead_code)]
extern "C" {
    pub fn thor_ghostty_ensure_app() -> bool;
    pub fn thor_ghostty_surface_new(
        nsview: *mut c_void,
        cwd: *const c_char,
        command: *const c_char,
        scale_factor: f64,
    ) -> ThorSurface;
    pub fn thor_ghostty_surface_set_frame(surface: ThorSurface, x: f64, y: f64, w: f64, h: f64);
    pub fn thor_ghostty_surface_set_hidden(surface: ThorSurface, hidden: bool);
    pub fn thor_ghostty_surface_set_size(surface: ThorSurface, width_px: u32, height_px: u32);
    pub fn thor_ghostty_surface_set_content_scale(surface: ThorSurface, x: f64, y: f64);
    pub fn thor_ghostty_surface_set_focus(surface: ThorSurface, focused: bool);
    pub fn thor_ghostty_surface_process_exited(surface: ThorSurface) -> bool;
    pub fn thor_ghostty_surface_draw(surface: ThorSurface);
    pub fn thor_ghostty_surface_free(surface: ThorSurface);
    pub fn thor_ghostty_app_tick();
    pub fn thor_ghostty_kill_all();
    pub fn thor_ghostty_surface_send_text(
        surface: ThorSurface,
        utf8: *const c_char,
        len: usize,
    );
    pub fn thor_ghostty_surface_read_screen(
        surface: ThorSurface,
        out: *mut c_char,
        cap: usize,
    ) -> usize;
    pub fn thor_ghostty_draw_count() -> u64;
    pub fn thor_ghostty_test_ime_compose(
        surface: ThorSurface,
        marked: *const c_char,
        final_: *const c_char,
    ) -> bool;
    pub fn thor_ghostty_test_type_key(
        surface: ThorSurface,
        characters: *const c_char,
        keycode: u16,
    ) -> bool;
    pub fn thor_ghostty_test_last_key_text() -> *const c_char;
    pub fn thor_ghostty_test_last_key_composing() -> bool;
}
