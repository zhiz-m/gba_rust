//use log::info;

use crate::{
    apu::{Apu, SoundBufferIt},
    bus::Bus,
    config,
    input_handler::{InputHandler, KeyInput},
    ppu::{Ppu, ScreenBuffer},
};

// smaller values have priority.
#[derive(Clone, Copy)]
enum Workflow {
    Timer = 0,
    Cpu = 1,
    Apu = 2,
    Ppu = 3,
    Normaliser = 4,
}

pub struct GBA {
    bus: Bus,
    //cpu: CPU,
    ppu: Ppu,
    input_handler: InputHandler,

    save_state: Vec<Vec<u8>>,
    save_state_updated: bool,

    //heap: BinaryHeap<Reverse<(u32, Workflow)>>,
    workflow_times: [(u32, Workflow); 5],
    //time_until_non_cpu_execution: u32,
    last_finished_time: u64,  // microseconds, continuous time
    last_fps_print_time: u64, // microseconds

    frame_counter: u32, // this is used to for counting; it is sometimes reset to 0
    total_frames_passed: u64, // this is always increasing
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
        let apu = Apu::new(audio_sample_rate);

        let save_state =
            save_state.unwrap_or_else(|| vec![vec![0; 128 * 1024]; config::NUM_SAVE_STATES]);
        let initial_save_state = save_state_bank.map(|x| save_state[x].as_slice());

        GBA {
            bus: Bus::new(
                bios_bin,
                rom_bin,
                initial_save_state,
                cartridge_type_str,
                apu,
            ),
            //cpu: CPU::new(),
            ppu: Ppu::new(),
            input_handler: InputHandler::new(),

            save_state,
            save_state_updated: false,

            workflow_times: [
                (0, Workflow::Timer),
                (0, Workflow::Cpu),
                (0, Workflow::Apu),
                (0, Workflow::Ppu),
                (0, Workflow::Normaliser),
            ],
            //time_until_non_cpu_execution: 0,

            //last_finished_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            //last_fps_print_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            last_finished_time: 0,
            last_fps_print_time: 0,

            frame_counter: 0,
            fps: None,
            total_frames_passed: 0,

            started: false,
        }

        // zero out input registers (NOTE: handled by BIOS)
        //res.input_handler.process_input(&res.key_receiver, &mut res.bus);
    }

    pub fn has_started(&self) -> bool {
        self.started
    }

    // todo: this is not a pure function despite its name. this should be changed
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

    pub fn get_save_state(&self) -> &[Vec<u8>] {
        &self.save_state
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
            let mut cur_min = 100_000_000;
            let mut cur_ans = Workflow::Timer;
            for x in self.workflow_times.iter() {
                if x.0 < cur_min {
                    cur_min = x.0;
                    cur_ans = x.1;
                }
            }

            match cur_ans {
                Workflow::Timer => {
                    self.bus.timer_clock();
                    self.workflow_times[0].0 += config::TIMER_CLOCK_INTERVAL_CLOCKS;
                }
                Workflow::Cpu => {
                    self.workflow_times[1].0 += self.bus.cpu_clock();
                }
                Workflow::Apu => {
                    self.bus.apu_clock();
                    self.workflow_times[2].0 += config::AUDIO_SAMPLE_CLOCKS;
                }
                Workflow::Ppu => {
                    self.workflow_times[3].0 += self.ppu.clock(&mut self.bus);
                    if self.ppu.buffer_ready {
                        self.on_new_buffer(current_time);

                        //info!("arm count: {}, thumb count: {}", self.bus.cpu.arm_cnt, self.bus.cpu.thumb_cnt);

                        return Ok(if self.last_finished_time > current_time {
                            self.last_finished_time - current_time
                        } else {
                            0
                        });
                    }
                }
                Workflow::Normaliser => {
                    if !self.input_handler.cur_speedup_state {
                        self.last_finished_time += config::CPU_EXECUTION_INTERVAL_US;
                    }

                    self.frame_counter += 1;
                    self.total_frames_passed += 1;

                    if self.frame_counter == config::FPS_RECORD_INTERVAL {
                        let since = current_time - self.last_fps_print_time;
                        if since > 0 {
                            let fps = config::FPS_RECORD_INTERVAL as f64 * 1000000. / since as f64;
                            self.fps = Some(fps);
                            self.last_fps_print_time = current_time;
                            #[cfg(feature = "print_cps")]
                            info!("frames per second: {:#.3}", fps);
                        }
                        self.frame_counter = 0;
                    }
                    #[cfg(feature = "debug_instr")]
                    {
                        self.bus.cpu.debug_cnt += 50;
                    }

                    // roughly every second in real-time, we want to normalize all the values in the array
                    if self.workflow_times[4].0 >= config::CPU_EXECUTION_INTERVAL_CLOCKS * 60 {
                        let min = self.workflow_times[4].0;
                        self.workflow_times.iter_mut().for_each(|x| x.0 -= min);
                        self.workflow_times[4].0 = config::CPU_EXECUTION_INTERVAL_CLOCKS;
                    } else {
                        self.workflow_times[4].0 += config::CPU_EXECUTION_INTERVAL_CLOCKS;
                    }
                }
            }
        }
    }

    // perform some IO
    // todo: maybe decouple IO handling from this.
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
        }
        for i in 0..config::NUM_SAVE_STATES {
            if self.input_handler.save_requested[i] {
                self.bus.export_sram(&mut self.save_state[i]);
                self.input_handler.save_requested[i] = false;
                self.save_state_updated = true;
            }
        }
    }

    pub fn total_frames_passed(&self) -> u64 {
        self.total_frames_passed
    }
}
