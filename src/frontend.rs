use std::collections::HashMap;
use std::sync::mpsc::{Receiver};
use std::time::Duration;

use glutin_window::GlutinWindow as Window;
use graphics::{clear, Transformed, rectangle};
use opengl_graphics::{GlGraphics, OpenGL};
use piston::{PressEvent, Key, Button, ReleaseEvent};
use piston::event_loop::{EventSettings, Events};
use piston::input::{RenderEvent};
use piston::window::WindowSettings;

use super::{
    ppu::{
        ScreenBuffer,  
    },
    key_input::{
        KeyBuffer, KeyInput
    }
};

pub struct Frontend{
    gl: Option<GlGraphics>,
    window: Option<Window>,
    events: Option<Events>,
    title: String,

    screenbuf_receiver: Receiver<ScreenBuffer>,
    last_screenbuf: ScreenBuffer,

    keybuf: KeyBuffer,
    key_map: HashMap<Key, KeyInput>,
}

impl Frontend{
    pub fn new(title: String, screenbuf_receiver: Receiver<ScreenBuffer>) -> Frontend{
        Frontend { 
            gl: None,
            window: None,
            events: None,
            title,

            screenbuf_receiver,
            last_screenbuf: ScreenBuffer::new(),

            keybuf: KeyBuffer::new(),
            key_map: HashMap::from([
                (Key::Z, KeyInput::A),
                (Key::X, KeyInput::B),
                (Key::Q, KeyInput::Select),
                (Key::W, KeyInput::Start),
                (Key::A, KeyInput::L),
                (Key::S, KeyInput::R),
                (Key::Up, KeyInput::Up),
                (Key::Down, KeyInput::Down),
                (Key::Right, KeyInput::Right),
                (Key::Left, KeyInput::Left),
            ]),
        }
    }
    
    pub fn start(&mut self) -> Result<(), &'static str>{
        self.window = Some(WindowSettings::new(&self.title, [480, 320])
            .graphics_api(OpenGL::V3_2)
            .exit_on_esc(true)
            .build()
            .unwrap());
        self.gl = Some(GlGraphics::new(OpenGL::V3_2));
        self.events = Some(Events::new(EventSettings::new()));

        while self.render().unwrap() {

        }

        return Ok(())
    }
    
    pub fn render(&mut self) -> Result<bool, &'static str>{
        if let Some(e) = self.events.as_mut().unwrap().next(self.window.as_mut().unwrap()){
            if let Ok(buf) = self.screenbuf_receiver.recv_timeout(Duration::from_millis(1)) {
                self.last_screenbuf = buf;
            }
            if let Some(args) = e.render_args(){
                let square = rectangle::square(0.0, 0.0, 2.);
                
                self.gl.as_mut().unwrap().draw(args.viewport(), |c, gl| {
                    clear([0., 0., 0., 1.], gl);
                    
                    for j in 0..160{
                        for i in 0..240{
                            let transform = c
                                .transform
                                .trans(i as f64 * 2., j as f64 * 2.);
                            let pixel = self.last_screenbuf.read_pixel(j, i).to_float();
                            rectangle([pixel.0, pixel.1, pixel.2, 1.], square, transform, gl);
                        }
                    }
                });
            }
            if let Some(Button::Keyboard(key)) = e.press_args(){
                if let Some(key_input) = self.key_map.get(&key) {
                    self.keybuf.press_key(*key_input);
                }
            }
            if let Some(Button::Keyboard(key)) = e.release_args(){
                if let Some(key_input) = self.key_map.get(&key) {
                    self.keybuf.release_key(*key_input);
                }
            }
            return Ok(true);
        }
        return Ok(false);
    }
}