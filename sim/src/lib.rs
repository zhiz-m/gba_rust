use std::collections::{LinkedList, VecDeque};

use gba_core::KeyInput;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Clone, Copy)]
struct KeyInputSerde(KeyInput);

impl Serialize for KeyInputSerde {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let output: u8 = self.0.into();
        output.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyInputSerde {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let output: Result<KeyInput, ()> = u8::deserialize(deserializer)?.try_into();
        Ok(KeyInputSerde(output.unwrap()))
    }
}

impl From<KeyInputSerde> for KeyInput {
    fn from(val: KeyInputSerde) -> Self {
        val.0
    }
}

impl From<KeyInput> for KeyInputSerde {
    fn from(other: KeyInput) -> KeyInputSerde {
        Self(other)
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct FrameInfo {
    frame: u64,
    current_time: u64,
    key_input: LinkedList<(KeyInputSerde, bool)>,
}

// remove default
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    rom_path: String,
    save: Option<(Vec<Vec<u8>>, usize)>,
    start_time: u64,
    frame_info: VecDeque<FrameInfo>,
}

#[derive(Clone)]
pub struct StateLogger {
    state: State,
    next_expected_frame: u64,
}

impl StateLogger {
    pub fn new(rom_path: String, save: Option<(Vec<Vec<u8>>, usize)>) -> StateLogger {
        StateLogger {
            state: State {
                rom_path,
                save,
                start_time: 0,
                frame_info: VecDeque::new(),
            },
            next_expected_frame: 0,
        }
    }

    pub fn init(&mut self, current_time: u64) {
        self.state.start_time = current_time
    }

    pub fn log_frame(&mut self, triggering_frame: u64, current_time: u64) {
        // assert!(triggering_frame == self.next_expected_frame);
        self.next_expected_frame += 1;
        self.state.frame_info.push_back(FrameInfo {
            frame: triggering_frame,
            current_time,
            key_input: LinkedList::new(),
        });
    }

    pub fn log_key_input_for_current_frame(&mut self, key_input: KeyInput, is_pressed: bool) {
        let frame_info = self.state.frame_info.back_mut().unwrap();
        frame_info
            .key_input
            .push_back((key_input.into(), is_pressed));
    }

    pub fn finalize(self) -> State {
        self.state
    }
}

pub mod sim {
    use core::str;
    use std::fs::{read, write};
    use std::time::{Duration, SystemTime};
    use std::{env, u64};

    use gba_core::ScreenBuffer;

    use crate::State;

    fn print_histogram(items: &mut [Duration]) {
        items.sort();
        let len = items.len() as f64;
        let of = |mult| (len * mult) as usize;
        let buckets = [("min", 1),
            ("25%", of(0.25)),
            ("50%", of(0.5)),
            ("75%", of(0.75)),
            ("max", items.len() - 1)];
        buckets.iter().for_each(|(str, _)| print!("|{: >12}", str));
        println!("|");
        buckets.iter().for_each(|(_, ind)| {
            let time = items[*ind];
            print!("|{:10}us", time.as_micros())
        });
        println!("|");
        // println!("{:?}", buckets);
    }

    pub fn save_state(state: &State, path: &str) {
        let result = bitcode::serialize(state).unwrap();
        write(path, result).unwrap()
    }

    pub fn load_state(path: &str) -> State {
        let bytes = read(path).unwrap();
        bitcode::deserialize(&bytes).unwrap()
    }

    fn img_get(screen_buffer: &ScreenBuffer) -> image::RgbImage {
        use image::{Rgb, RgbImage};
        let width = 240;
        let height = 160;
        let mut img = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel = screen_buffer.read_pixel(y as usize, x as usize).to_u8();
                img.put_pixel(x, y, Rgb([pixel.0, pixel.1, pixel.2]))
            }
        }
        img
    }

    pub fn drive_gba_from_state(mut state: State) -> image::RgbImage {
        let bios_path =
            env::var("GBA_RUST_BIOS_PATH").expect("Env variable GBA_RUST_BIOS_PATH not found");
        let bios_bin = read(bios_path).expect("did not find BIOS file");
        let rom_bin = read(state.rom_path).expect("did not find ROM");
        let (save_bin, save_state_bank) = match state.save {
            Some((save_bin, save_state_bin)) => (Some(save_bin), Some(save_state_bin)),
            None => (None, None),
        };
        let mut gba =
            gba_core::GBA::new(&bios_bin, &rom_bin, save_bin, save_state_bank, None, 4800);
        gba.init(state.start_time);

        let start_time = SystemTime::now();
        let mut time = start_time;
        let mut times = Vec::with_capacity(state.frame_info.len());
        let mut prev_frame = 0;
        let mut screen_buffer = None;

        while let Some(frame_info) = state.frame_info.pop_front() {
            if gba.total_frames_passed() != frame_info.frame {
                println!("{} {}", gba.total_frames_passed(), frame_info.frame);
                assert!(false);
            }
            let _sleep_micros: u64 = gba.process_frame(frame_info.current_time).unwrap();
            let next_time = SystemTime::now();
            let frame_diff = if prev_frame == 0 {
                1
            } else {
                frame_info.frame - prev_frame
            };
            prev_frame = frame_info.frame;
            let diff = next_time.duration_since(time).unwrap() / frame_diff as u32;
            time = next_time;
            times.push(diff);

            if let Some(buf) = gba.get_screen_buffer() {
                screen_buffer = Some(buf.clone())
            }
            if gba.get_sound_buffer().is_some() {
                gba.reset_sound_buffer();
            }
            gba.input_frame_preprocess();
            frame_info
                .key_input
                .into_iter()
                .for_each(|(key_input, is_pressed)| {
                    gba.process_key(key_input.into(), is_pressed);
                })
        }

        let total_time = SystemTime::now()
            .duration_since(start_time)
            .unwrap()
            .as_millis();

        println!("total time: {total_time}ms");
        println!("time per frame");

        print_histogram(&mut times);
        println!("amortized fps: {}", prev_frame * 1000 / total_time as u64);
        img_get(&screen_buffer.unwrap())
    }
}
