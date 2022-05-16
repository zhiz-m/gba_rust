use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use glutin_window::GlutinWindow as Window;
use graphics::{clear, Transformed, rectangle};
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::{RenderEvent};
use piston::window::WindowSettings;

#[derive(Clone, Copy)]
pub struct Pixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Pixel{
    pub fn new(r: f32, g: f32, b: f32) -> Pixel{
        //if r > 0. || b > 0. || g > 0.{
        //    print!("colour");
        //}
        return Pixel { r, g, b }
    }
}

#[derive(Clone)]
pub struct ScreenBuffer {
    buffer: Vec<Vec<Pixel>>,
}

impl ScreenBuffer{
    pub fn new() -> ScreenBuffer{
        return ScreenBuffer{
            buffer: vec![vec![Pixel::new(0.,0.,0.); 240]; 160],
        }
    }
    pub fn write_pixel(&mut self, row: usize, col: usize, pixel: Pixel){
        self.buffer[row][col] = pixel;
    }
    pub fn read_pixel(&self, row: usize, col: usize) -> Pixel{
        return self.buffer[row][col];
    }
}

pub struct Frontend{
    gl: Option<GlGraphics>,
    window: Option<Window>,
    events: Option<Events>,
    title: String,
    buff_receiver: Receiver<ScreenBuffer>,
    last_buff: ScreenBuffer,
}

impl Frontend{
    pub fn new(title: String, buff_receiver: Receiver<ScreenBuffer>) -> Frontend{
        Frontend { 
            gl: None,
            window: None,
            events: None,
            title,
            buff_receiver,
            last_buff: ScreenBuffer::new()
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
            if let Ok(buf) = self.buff_receiver.recv_timeout(Duration::from_millis(1)) {
                self.last_buff = buf;
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
                            let pixel = self.last_buff.read_pixel(j, i);
                            rectangle([pixel.r, pixel.g, pixel.b, 1.], square, transform, gl);
                        }
                    }
                });
            }
            return Ok(true);
        }
        return Ok(false);
    }
}