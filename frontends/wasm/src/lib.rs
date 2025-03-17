use std::convert::TryInto;

use gba_core::{marshall_save_state, KeyInput, GBA};
use js_sys::{Float32Array, Uint8Array};
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::CanvasRenderingContext2d;

#[wasm_bindgen]
pub struct GbaWasm {
    gba: GBA,
    raw_screen_buffer: Vec<u8>,
}

#[wasm_bindgen]
impl GbaWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(
        bios_bin: &[u8],
        rom_bin: &[u8],
        save_state: Option<Uint8Array>,
        save_state_bank: Option<u32>,
        sample_rate: f32,
    ) -> GbaWasm {
        // let x = marshall_save_state(save_state);
        GbaWasm {
            gba: GBA::new(
                bios_bin,
                rom_bin,
                save_state.map(|x| marshall_save_state(&x.to_vec())),
                save_state_bank.map(|x| x as usize),
                None,
                sample_rate as usize,
            ),
            raw_screen_buffer: vec![0u8; 4 * 320 * 480],
        }
    }

    pub fn process_frame(&mut self, current_time: u64) -> Result<u64, JsValue> {
        let micros = self
            .gba
            .process_frame(current_time)
            .map_err(Into::<JsValue>::into)?;

        Ok(micros)
    }

    pub fn display_picture(
        &mut self,
        canvas_context: &CanvasRenderingContext2d,
    ) -> Result<(), JsValue> {
        // video
        if let Some(screen_buffer) = self.gba.get_screen_buffer() {
            for i in 0..320 {
                for j in 0..480 {
                    let ind = i * 480 + j;
                    let pixel = screen_buffer.read_pixel(i >> 1, j >> 1).to_u8();
                    self.raw_screen_buffer[ind << 2] = pixel.0;
                    self.raw_screen_buffer[(ind << 2) + 1] = pixel.1;
                    self.raw_screen_buffer[(ind << 2) + 2] = pixel.2;
                    self.raw_screen_buffer[(ind << 2) + 3] = 255;
                }
            }
            let data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
                Clamped(&self.raw_screen_buffer[..]),
                480,
                320,
            )?;
            canvas_context.put_image_data(&data, 0., 0.)?;
        }

        Ok(())
    }

    pub fn input_frame_preprocess(&mut self) {
        self.gba.input_frame_preprocess();
    }

    pub fn key_input(&mut self, key: u8, is_pressed: bool) {
        if let Ok(key) = TryInto::<KeyInput>::try_into(key) {
            self.gba.process_key(key, is_pressed);
        }
        // todo
    }

    // interwoven
    pub fn get_audio_buffer(&mut self) -> Option<Float32Array> {
        let it = self.gba.get_sound_buffer()?;
        let mut ret = Vec::with_capacity(it.len() * 2);
        for (a, b) in it {
            ret.push(a);
            ret.push(b);
        }
        self.gba.reset_sound_buffer();
        Some(ret[..].into())
    }

    pub fn get_fps(&mut self) -> Option<f64> {
        self.gba.get_fps()
    }

    pub fn init(&mut self, current_time: u64) {
        self.gba.init(current_time)
    }

    pub fn get_save_state(&self) -> Uint8Array {
        self.gba.get_save_state()[..].concat()[..].into()
    }
}
