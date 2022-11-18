mod util;

use std::{cell::RefCell, rc::Rc};

use util::HtmlState;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::AudioContext;
// When the `wee_alloc` feature is enabled, this uses `wee_alloc` as the global
// allocator.
//
// If you don't want to use `wee_alloc`, you can safely delete this.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// This is like the `main` function, except for JavaScript.
#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    let window = web_sys::window().unwrap();

    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gba_rust_canvas")
        .expect("canvas not found")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();
    canvas.set_width(480);
    canvas.set_height(320);

    let audio_context = AudioContext::new()?;
    let source = audio_context.create_buffer_source()?;
    let audio_buffer = audio_context.create_buffer(2, 10, 48000f32)?;
    source.set_buffer(Some(&audio_buffer));
    source
        .connect_with_audio_node(&audio_context.destination())
        .unwrap();

    let bios = Rc::new(RefCell::new(None));
    let rom = Rc::new(RefCell::new(None));
    let save_state = Rc::new(RefCell::new(None));
    let save_bank = Rc::new(RefCell::new(None));

    util::configure_file_input("bios_input", bios.clone())?;
    util::configure_file_input("rom_input", rom.clone())?;
    util::configure_file_input("save_state_input", save_state.clone())?;
    util::configure_int_input("save_bank_input", save_bank.clone())?;

    let html_state = HtmlState {
        raw_screen_buffer: vec![0u8; 4 * 320 * 480],
        fps_label: document
            .get_element_by_id("fps_label")
            .unwrap()
            .dyn_into::<web_sys::HtmlDivElement>()?,
        canvas_context: canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?,
    };
    let gba = Rc::new(RefCell::new(None));
    util::configure_reset_button(
        "reset_button",
        gba,
        bios,
        save_state,
        save_bank,
        rom,
        Rc::new(RefCell::new(html_state)),
    )?;

    Ok(())
}
