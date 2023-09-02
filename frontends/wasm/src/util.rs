use gba_core::{marshall_save_state, GBA, NUM_SAVE_STATES};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlDivElement, AudioContext};

pub struct HtmlState {
    pub raw_screen_buffer: Vec<u8>,
    pub fps_label: HtmlDivElement,
    pub canvas_context: CanvasRenderingContext2d,
}

pub fn configure_file_input(id: &str, output: Rc<RefCell<Option<Vec<u8>>>>) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let input = document
        .get_element_by_id(id)
        .unwrap()
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap();

    let closure = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let element = e
            .target()
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();

        let file_reader = web_sys::FileReader::new().unwrap();
        if let Some(file) = &element.files().unwrap().get(0) {
            file_reader.read_as_array_buffer(&file).unwrap();
            let output_cloned = Rc::clone(&output);
            let onload = Closure::wrap(Box::new(move |event: web_sys::Event| {
                let file_reader: web_sys::FileReader = event.target().unwrap().dyn_into().unwrap();
                let psd = file_reader.result().unwrap();
                let psd = js_sys::Uint8Array::new(&psd);

                let mut r = output_cloned.borrow_mut();

                let mut psd_file = vec![0; psd.length() as usize];
                psd.copy_to(&mut psd_file);
                *r = Some(psd_file);
            }) as Box<dyn FnMut(_)>);

            file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
            onload.forget();
        }
    }) as Box<dyn FnMut(_)>);
    input.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

pub fn configure_int_input(id: &str, output: Rc<RefCell<Option<i32>>>) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let input = document
        .get_element_by_id(id)
        .unwrap()
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap();

    let closure = Closure::wrap(Box::new(move |e: web_sys::Event| {
        let element = e
            .target()
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();

        if let Ok(data) = element.value().parse::<i32>() {
            if 1 <= data && data <= NUM_SAVE_STATES as i32 {
                *(output.borrow_mut()) = Some(data);
            }
        }
    }) as Box<dyn FnMut(_)>);
    input.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

pub fn configure_reset_button(
    id: &str,
    gba: Rc<RefCell<Option<GBA>>>,
    bios: Rc<RefCell<Option<Vec<u8>>>>,
    save_state: Rc<RefCell<Option<Vec<u8>>>>,
    save_bank: Rc<RefCell<Option<i32>>>,
    rom: Rc<RefCell<Option<Vec<u8>>>>,
    html_state: Rc<RefCell<HtmlState>>,
) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    let button = document
        .get_element_by_id(id)
        .unwrap()
        .dyn_into::<web_sys::HtmlButtonElement>()
        .unwrap();

    let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
        if let Some(bios) = bios.borrow().as_ref() {
            if let Some(rom) = rom.borrow().as_ref() {
                let save_state = save_state
                    .borrow()
                    .as_ref()
                    .map(|data| marshall_save_state(data));
                *(gba.borrow_mut()) = Some(GBA::new(
                    bios,
                    rom,
                    save_state,
                    save_bank.borrow().as_ref().map(|x| *x as usize),
                    None,
                    48000,
                ));
                schedule_gba(gba.clone(), 500000, html_state.clone()).unwrap();
            }
        }
    }) as Box<dyn FnMut(_)>);
    button.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
    closure.forget();
    Ok(())
}

fn schedule_gba(
    gba_rc: Rc<RefCell<Option<GBA>>>,
    time_micros: u64,
    html_state: Rc<RefCell<HtmlState>>,
) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let closure = Closure::wrap(Box::new(move |_: web_sys::Event| {
        if let Some(gba) = gba_rc.borrow_mut().as_mut() {
            let html_state = html_state.clone();
            let mut html_state_inner = html_state.borrow_mut();

            //console::log_1(&"gba_frame_start".into());
            let time_micros = (js_sys::Date::now() * 1000.) as u64;
            if !gba.has_started() {
                gba.init(time_micros);
            }
            let sleep_micros = gba.process_frame(time_micros).unwrap();
            //let time_micros2 = (js_sys::Date::now() * 1000.) as u64;
            //console::log_2(&"frame time ms:".into(), &(time_micros2-time_micros).into());

            // video
            if let Some(screen_buffer) = gba.get_screen_buffer() {
                for i in 0..320 {
                    for j in 0..480 {
                        let ind = i * 480 + j;
                        let pixel = screen_buffer.read_pixel(i >> 1, j >> 1).to_u8();
                        html_state_inner.raw_screen_buffer[ind << 2] = pixel.0;
                        html_state_inner.raw_screen_buffer[(ind << 2) + 1] = pixel.1;
                        html_state_inner.raw_screen_buffer[(ind << 2) + 2] = pixel.2;
                        html_state_inner.raw_screen_buffer[(ind << 2) + 3] = 255;
                    }
                }
            }
            let data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
                Clamped(&mut html_state_inner.raw_screen_buffer[..]),
                480,
                320,
            )
            .unwrap();
            html_state_inner
                .canvas_context
                .put_image_data(&data, 0., 0.)
                .unwrap();

            // audio
            if let Some(it) = gba.get_sound_buffer(){
                let audio_data: (Vec<_>, Vec<_>) = it.unzip();
                let audio_context = AudioContext::new().unwrap();
                let source = audio_context.create_buffer_source().unwrap();
                let audio_buffer = audio_context.create_buffer(2, 10, 48000f32).unwrap();

                audio_buffer.copy_to_channel(&audio_data.0[..], 0).unwrap();
                audio_buffer.copy_to_channel(&audio_data.1[..], 1).unwrap();

                source.set_buffer(Some(&audio_buffer));
                source
                    .connect_with_audio_node(&audio_context.destination())
                    .unwrap();
                source.connect_with_audio_node(&audio_context.destination()).unwrap();
                source.start().unwrap();
                gba.reset_sound_buffer();
            }

            // saves
            // TODO

            // fps
            if let Some(fps) = gba.get_fps() {
                html_state_inner
                    .fps_label
                    .set_inner_text(&format!("FPS: {:.3}", fps));
            }

            // input
            gba.input_frame_preprocess();

            // schedule next render
            //console::log_1(&"gba_frame_end".into());
            // console::log_1(&(sleep_micros.to_string()).into()) ;
            schedule_gba(gba_rc.clone(), sleep_micros, html_state.clone()).unwrap();
        }
    }) as Box<dyn FnMut(_)>);
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            (time_micros / 1000) as i32,
        )
        .unwrap();
    closure.forget();
    Ok(())
}
