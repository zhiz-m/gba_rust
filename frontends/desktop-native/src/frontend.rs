use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Device;
use glutin_window::GlutinWindow as Window;
use graphics::{clear, rectangle, Transformed};
use opengl_graphics::{GlGraphics, OpenGL};
use piston::event_loop::{EventSettings, Events};
use piston::input::RenderEvent;
use piston::window::WindowSettings;
use piston::{Button, Key, PressEvent, ReleaseEvent};

use gba_core::{KeyInput, ScreenBuffer};

pub struct Frontend {
    gl: Option<GlGraphics>,
    window: Option<Window>,
    events: Option<Events>,
    title: String,

    screenbuf_receiver: Receiver<ScreenBuffer>,
    last_screenbuf: ScreenBuffer,

    key_map: HashMap<Key, KeyInput>,
    key_sender: Sender<(KeyInput, bool)>,

    audio_output_device: Device,
    audio_receiver: Option<Receiver<(f32, f32)>>,

    fps_receiver: Receiver<f64>,
    cur_fps: f64,
    avg_fps: f64,
}

impl Frontend {
    pub fn new(
        title: String,
        screenbuf_receiver: Receiver<ScreenBuffer>,
        key_sender: Sender<(KeyInput, bool)>,
        audio_receiver: Receiver<(f32, f32)>,
        fps_receiver: Receiver<f64>,
    ) -> Frontend {
        let audio_output_device = cpal::default_host()
            .devices()
            .unwrap()
            .map(|x| {
                if x.default_output_config().ok()?.channels() == 2 {
                    Some(x)
                } else {
                    None
                }
            })
            .find(|x| x.is_some())
            .expect("no suitable stereo output device")
            .unwrap();
        println!("audio device: {}", &audio_output_device.name().unwrap());
        Frontend {
            gl: None,
            window: None,
            events: None,
            title,

            screenbuf_receiver,
            last_screenbuf: ScreenBuffer::new(),

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
                (Key::Space, KeyInput::Speedup),
                (Key::D1, KeyInput::Save0),
                (Key::D2, KeyInput::Save1),
                (Key::D3, KeyInput::Save2),
                (Key::D4, KeyInput::Save3),
                (Key::D5, KeyInput::Save4),
            ]),
            key_sender,

            audio_output_device,
            audio_receiver: Some(audio_receiver),

            fps_receiver,
            cur_fps: 60f64,
            avg_fps: 60f64,
        }
    }

    pub fn get_sample_rate(&self) -> usize {
        let config = self.audio_output_device.default_output_config().unwrap();
        config.sample_rate().0 as usize
    }

    pub fn start(&mut self) -> Result<(), &'static str> {
        self.window = Some(
            WindowSettings::new(&self.title, [480, 320])
                .graphics_api(OpenGL::V3_2)
                .exit_on_esc(true)
                .build()
                .unwrap(),
        );
        self.gl = Some(GlGraphics::new(OpenGL::V3_2));
        self.events = Some(Events::new(EventSettings::new()));
        let config = self
            .audio_output_device
            .default_output_config()
            .unwrap()
            .into();
        let receiver = self.audio_receiver.take().unwrap();
        //let mut t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let stream = self
            .audio_output_device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let channel_num = config.channels as usize;
                    //println!("data len: {}, channel num: {}", data.len(), channel_num);
                    //let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    //let since = now.checked_sub(t).unwrap().as_nanos();
                    //t = now;
                    //println!("nanos since: {}", since);
                    for frame in data.chunks_mut(channel_num) {
                        match receiver.recv() {
                            Ok(stereo_data) => {
                                for stereo_frame in frame.chunks_mut(2) {
                                    stereo_frame[0] = stereo_data.0;
                                    stereo_frame[1] = stereo_data.1;
                                }
                            }
                            Err(why) => {
                                //println!("audio stream err: {}", why.to_string())
                            }
                        }
                    }
                },
                move |err| println!("err: {}", err.to_string()),
            )
            .unwrap();

        stream.play().unwrap();

        while self.render().unwrap() {}

        return Ok(());
    }

    pub fn render(&mut self) -> Result<bool, &'static str> {
        if let Some(e) = self
            .events
            .as_mut()
            .unwrap()
            .next(self.window.as_mut().unwrap())
        {
            while let Ok(buf) = self.screenbuf_receiver.try_recv() {
                self.last_screenbuf = buf;
            }
            while let Ok(fps) = self.fps_receiver.try_recv() {
                self.cur_fps = fps;
                self.avg_fps = self.avg_fps * 0.8 + 0.2 * self.cur_fps;
                self.window
                    .as_ref()
                    .unwrap()
                    .ctx
                    .window()
                    .set_title(&format!(
                        "{} | FPS ({:5.3},{:5.3})",
                        self.title, self.cur_fps, self.avg_fps
                    ));
            }
            if let Some(args) = e.render_args() {
                let square = rectangle::square(0.0, 0.0, 2.);

                self.gl.as_mut().unwrap().draw(args.viewport(), |c, gl| {
                    clear([0., 0., 0., 1.], gl);

                    for j in 0..160 {
                        for i in 0..240 {
                            let transform = c.transform.trans(i as f64 * 2., j as f64 * 2.);
                            let pixel = self.last_screenbuf.read_pixel(j, i).to_float();
                            rectangle([pixel.0, pixel.1, pixel.2, 1.], square, transform, gl);
                        }
                    }
                });
            }
            if let Some(Button::Keyboard(key)) = e.press_args() {
                if let Some(key_input) = self.key_map.get(&key) {
                    if let Err(why) = self.key_sender.send((*key_input, true)) {
                        println!("   keybuf sending error: {}", why.to_string());
                    }
                }
            }
            if let Some(Button::Keyboard(key)) = e.release_args() {
                if let Some(key_input) = self.key_map.get(&key) {
                    if let Err(why) = self.key_sender.send((*key_input, false)) {
                        println!("   keybuf sending error: {}", why.to_string());
                    }
                }
            }
            return Ok(true);
        }
        return Ok(false);
    }
}
