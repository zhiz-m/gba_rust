use crate::{
    apu::{SoundBufferIt, APU},
    bus::Bus,
    config,
    cpu::CPU,
    input_handler::{InputHandler, KeyInput},
    ppu::{ScreenBuffer, PPU},
};

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use std::{cmp::Reverse, collections::BinaryHeap};

// smaller values have priority.
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
enum Workflow {
    Timer = 0,
    CPU = 1,
    APU = 2,
    PPU = 3,
    Normaliser = 4,
}

pub struct GBA {
    bus: Bus,
    //cpu: CPU,
    ppu: PPU,
    input_handler: InputHandler,

    save_state: Vec<Vec<u8>>,
    save_state_updated: bool,

    //heap: BinaryHeap<Reverse<(u32, Workflow)>>,
    workflow_times: [(u32, Workflow); 5],
    time_until_non_cpu_execution: u32,

    last_finished_time: u64, // microseconds, continuous time
    last_fps_print_time: u64, // microseconds

    frame_counter: u32,
    fps: Option<f64>,

    started: bool,
}

impl GBA {
    pub fn new(
        bios_bin: &[u8],
        rom_bin: &[u8],
        save_state: Option<Vec<Vec<u8>>>,
        save_state_bank: Option<usize>,
        cartridge_type_str: Option<&str>,
        audio_sample_rate: usize,
    ) -> GBA {
        let apu = APU::new(audio_sample_rate);

        /*let rom_save_path = match rom_save_path {
            Some(path) => path.to_string(),
            None => {
                let save_state_dir = Path::new(&rom_path).parent().unwrap().to_str().expect("invalid rom path").to_string() + config::SAVE_FILE_DIR;
                fs::create_dir_all(&save_state_dir).unwrap();
                let rom_path_filename = Path::new(&rom_path).file_name().unwrap().to_str().unwrap().to_string();
                println!("save_state_dir: {}, rom_path_filename: {}", save_state_dir, rom_path_filename);
                let rom_save_path = if rom_path_filename.contains("."){
                    let pos = rom_path_filename.rfind(".").unwrap();
                    if pos != 0{
                        format!("{}{}", &rom_path_filename[0..pos], config::SAVE_FILE_SUF)
                    }
                    else{
                        format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
                    }
                }
                else{
                    format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
                };
                save_state_dir + "/" + &rom_save_path
            }
        };
        println!("rom save path: {}", rom_save_path);

        let mut save_state = vec![vec![0; 128*1024]; config::NUM_SAVE_STATES];
        // read save path into save_state
        if Path::new(&rom_save_path).exists() {
            let mut reader = BufReader::new(File::open(&rom_save_path).unwrap());
            for i in 0..config::NUM_SAVE_STATES{
                reader.read(&mut save_state[i]).unwrap();
            }
        }*/
        /*let save_state = save_state.map(
        |x| x.chunks(x.len() / config::NUM_SAVE_STATES).map(
            |x| x.to_vec()
        ).collect())*/
        let save_state = save_state.unwrap_or(vec![vec![0; 128 * 1024]; config::NUM_SAVE_STATES]);
        let initial_save_state = match save_state_bank {
            None => None,
            Some(bank) => Some(save_state[bank - 1].as_slice()),
        };

        let res = GBA {
            bus: Bus::new(
                bios_bin,
                rom_bin,
                initial_save_state,
                cartridge_type_str,
                apu,
            ),
            //cpu: CPU::new(),
            ppu: PPU::new(),
            input_handler: InputHandler::new(),

            save_state,
            save_state_updated: false,

            /*heap: BinaryHeap::from([
                Reverse((0, Workflow::Timer)),
                Reverse((0, Workflow::CPU)),
                Reverse((0, Workflow::APU)),
                Reverse((0, Workflow::PPU)),
                Reverse((0, Workflow::Normaliser)),
            ]),*/
            workflow_times: [
                (0, Workflow::Timer),
                (0, Workflow::CPU),
                (0, Workflow::APU),
                (0, Workflow::PPU),
                (0, Workflow::Normaliser),
            ],
            time_until_non_cpu_execution: 0,

            //last_finished_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            //last_fps_print_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            
            last_finished_time: 0,
            last_fps_print_time: 0,

            frame_counter: 0,
            fps: None,

            started: false,
        };

        // zero out input registers (NOTE: handled by BIOS)
        //res.input_handler.process_input(&res.key_receiver, &mut res.bus);

        res
    }

    pub fn has_started(&self) -> bool{
        self.started
    }

    pub fn get_screen_buffer(&mut self) -> Option<&ScreenBuffer> {
        self.ppu.get_screen_buffer()
    }

    pub fn get_sound_buffer(&mut self) -> Option<SoundBufferIt> {
        self.bus.apu.get_audio_buffer()
    }

    pub fn reset_sound_buffer(&mut self) {
        self.bus.apu.clear_buffer();
    }

    pub fn get_updated_save_state(&mut self) -> Option<&[Vec<u8>]> {
        if self.save_state_updated {
            self.save_state_updated = false;
            Some(&self.save_state)
        } else {
            None
        }
    }

    pub fn get_fps(&mut self) -> Option<f64> {
        self.fps.take()
    }

    // must be called prior to updating keys in each frame
    pub fn input_frame_preprocess(&mut self) {
        self.input_handler.frame_preprocess()
    }

    pub fn process_key(&mut self, key: KeyInput, is_pressed: bool) {
        self.input_handler.process_key(key, is_pressed);
    }

    pub fn init(&mut self, current_time: u64) {
        self.last_finished_time = current_time;
        self.last_fps_print_time = current_time;
        self.frame_counter = 0;
        self.started = true;
    }

    /// on successful frame, returns the number of microseconds that the emulator clock is ahead of the supposed true GBA clock
    pub fn process_frame(&mut self, current_time: u64) -> Result<u64, &'static str> {
        loop {
            match self.workflow_times.iter().min().unwrap().1 {
                Workflow::Timer => {
                    self.bus.timer_clock();
                    self.workflow_times[0].0 += config::TIMER_CLOCK_INTERVAL_CLOCKS;
                }
                Workflow::CPU => {
                    self.workflow_times[1].0 += self.bus.cpu_clock();
                }
                Workflow::APU => {
                    self.bus.apu_clock();
                    self.workflow_times[2].0 += config::AUDIO_SAMPLE_CLOCKS;
                }
                Workflow::PPU => {
                    self.workflow_times[3].0 += self.ppu.clock(&mut self.bus);
                    if self.ppu.buffer_ready {
                        self.on_new_buffer(current_time);
                        //println!("interrupts: {:#034b}", self.bus.read_word_raw(0x200, crate::bus::MemoryRegion::IO));
                        return Ok(if self.last_finished_time > current_time {self.last_finished_time - current_time} else {0});
                    }
                }
                Workflow::Normaliser => {
                    if !self.input_handler.cur_speedup_state {
                        self.last_finished_time += config::CPU_EXECUTION_INTERVAL_US;
                    }

                    self.frame_counter += 1;

                    if self.frame_counter == config::FPS_RECORD_INTERVAL {
                        let since = current_time - self.last_fps_print_time;
                        if since > 0 {
                            let fps =
                                config::FPS_RECORD_INTERVAL as f64 * 1000000. / since as f64;
                            self.fps = Some(fps);
                            self.last_fps_print_time = current_time;
                            #[cfg(feature = "print_cps")]
                            println!("frames per second: {:#.3}", fps);
                        }
                        self.frame_counter = 0;
                    }
                    #[cfg(feature = "debug_instr")]
                    {
                        self.bus.cpu.debug_cnt += 0;
                    }

                    // roughly every second in real-time, we want to normalize all the values in the array
                    if self.workflow_times[4].0 >= config::CPU_EXECUTION_INTERVAL_CLOCKS * 60 {
                        let min = self.workflow_times[4].0;
                        self.workflow_times.iter_mut().for_each(|x|(*x).0 -= min);
                        self.workflow_times[4].0 = config::CPU_EXECUTION_INTERVAL_CLOCKS;
                    } else {
                        self.workflow_times[4].0 += config::CPU_EXECUTION_INTERVAL_CLOCKS;
                    }
                }
                _ => unreachable!(),
            }
            /*let cur = self
                .heap
                .peek()
                .expect("logical error: binary heap is empty")
                .0;
            self.heap.pop().unwrap();
            match cur.1 {
                Workflow::Timer => {
                    self.bus.timer_clock();
                    self.heap.push(Reverse((
                        cur.0 + config::TIMER_CLOCK_INTERVAL_CLOCKS,
                        Workflow::Timer,
                    )));
                }
                Workflow::CPU => {
                    self.heap
                        .push(Reverse((cur.0 + self.bus.cpu_clock(), Workflow::CPU)));
                }
                Workflow::APU => {
                    self.bus.apu_clock();
                    self.heap.push(Reverse((
                        cur.0 + config::AUDIO_SAMPLE_CLOCKS,
                        Workflow::APU,
                    )));
                }
                Workflow::PPU => {
                    let clocks = self.ppu.clock(&mut self.bus);
                    self.heap.push(Reverse((cur.0 + clocks, Workflow::PPU)));
                    if self.ppu.buffer_ready {
                        self.on_new_buffer(current_time);
                        return Ok(if self.last_finished_time > current_time {self.last_finished_time - current_time} else {0});
                    }
                }
                Workflow::Normaliser => {
                    if !self.input_handler.cur_speedup_state {
                        self.last_finished_time += config::CPU_EXECUTION_INTERVAL_US;
                    }

                    self.frame_counter += 1;

                    if self.frame_counter == config::FPS_RECORD_INTERVAL {
                        let since = current_time - self.last_fps_print_time;
                        if since > 0 {
                            let fps =
                                config::FPS_RECORD_INTERVAL as f64 * 1000000. / since as f64;
                            self.fps = Some(fps);
                            self.last_fps_print_time = current_time;
                            #[cfg(feature = "print_cps")]
                            println!("frames per second: {:#.3}", fps);
                        }
                        self.frame_counter = 0;
                    }
                    #[cfg(feature = "debug_instr")]
                    {
                        self.bus.cpu.debug_cnt += 50;
                    }

                    // roughly every second in real-time, we want to normalize all the values in the heap
                    if cur.0 >= config::CPU_EXECUTION_INTERVAL_CLOCKS * 60 {
                        let mut items = vec![];
                        for item in self.heap.iter() {
                            items.push(Reverse((item.0 .0 - cur.0, item.0 .1)));
                        }
                        self.heap.clear();
                        for item in items.into_iter() {
                            self.heap.push(item);
                        }
                        self.heap.push(Reverse((
                            config::CPU_EXECUTION_INTERVAL_CLOCKS,
                            Workflow::Normaliser,
                        )));
                    } else {
                        self.heap.push(Reverse((
                            cur.0 + config::CPU_EXECUTION_INTERVAL_CLOCKS,
                            Workflow::Normaliser,
                        )));
                    }
                }
            }*/
        }
    }

    // perform some IO
    fn on_new_buffer(&mut self, current_time: u64) {
        // handle input once per frame
        //self.input_handler.process_input(&self.key_receiver, &mut self.bus);
        self.input_handler.commit(&mut self.bus);
        if self.input_handler.cur_speedup_state != self.input_handler.prev_speedup_state {
            self.bus.apu.extern_audio_enabled = self.input_handler.prev_speedup_state;
            if !self.input_handler.cur_speedup_state {
                //self.last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                self.last_finished_time = current_time;
                self.ppu.frame_count_render = 1;
            } else {
                self.ppu.frame_count_render = config::FRAME_RENDER_INTERVAL_SPEEDUP;
            }
            println!("arm_count: {}, arm_cache_miss_ratio: {}, thumb_count: {}", self.bus.cpu.arm_count, self.bus.cpu.arm_cache_miss as f32 / self.bus.cpu.arm_count as f32, self.bus.cpu.thumb_count);
        }
        for i in 0..config::NUM_SAVE_STATES {
            if self.input_handler.save_requested[i] {
                self.bus.export_sram(&mut self.save_state[i]);
                self.input_handler.save_requested[i] = false;
                self.save_state_updated = true;
            }
        }
    }
}
