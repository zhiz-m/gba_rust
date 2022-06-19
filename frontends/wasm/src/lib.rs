use std::{thread, sync::mpsc};

use gba_core::{GBA, ScreenBuffer};
use wasm_bindgen::prelude::*;
use web_sys::console;


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


    // Your code goes here!
    console::log_1(&JsValue::from_str("Hello world!"));

    let (tx1, rx1) = mpsc::channel();
    
    
    let (tx2, rx2) = mpsc::channel();
    
    // audio
    let (tx3, rx3) = mpsc::channel();

    

    // fps
    let (tx4, rx4) = mpsc::channel();
    
    let screenbuf_handler = move |screenbuf: ScreenBuffer|{
        if let Err(why) = tx1.send(screenbuf){
            println!("   screenbuf sending error: {}", why.to_string());
        }
    };
    
    let audio_handler = move |buf: &[Vec<f32>]|{
        //tx3.send((0f32,0f32)).unwrap();
        for j in 0..buf[0].len(){
            tx3.send((buf[0][j], buf[1][j])).unwrap();
        }
    };

    let mut gba = GBA::new("test", "test", Some("test"), Some(1), None, Box::new(screenbuf_handler), rx2, Box::new(audio_handler), 48000, tx4);

    thread::spawn(move || {
        gba.start().unwrap();
    });

    Ok(())
}
