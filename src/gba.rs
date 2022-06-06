
use crate::{bus::Bus, cpu::CPU, ppu::{PPU, ScreenBuffer}, input_handler::{InputHandler, KeyInput}, apu::APU, config};

use std::{thread, time:: {SystemTime, UNIX_EPOCH, Duration}, sync::mpsc::{Sender, Receiver}, io::{BufReader, Read, BufWriter, Write}, fs::File, path::Path};

pub struct GBA {
    bus: Bus,
    cpu: CPU,
    ppu: PPU,
    input_handler: InputHandler,

    screenbuf_sender: Sender<ScreenBuffer>,
    key_receiver: Receiver<(KeyInput,bool)>,
    fps_sender: Sender<f64>,

    rom_save_path: String,
    save_state: Vec<Vec<u8>>,
}

impl GBA {
    pub fn new(rom_path: String, rom_save_path: Option<String>, save_state_bank: Option<usize>, cartridge_type_str: Option<String>, screenbuf_sender: Sender<ScreenBuffer>, key_receiver: Receiver<(KeyInput,bool)>, audio_sender: Sender<(f32, f32)>, audio_sample_rate: usize, fps_sender: Sender<f64>) -> GBA {
        let apu = APU::new(audio_sample_rate, audio_sender);

        let rom_save_path = match rom_save_path {
            Some(path) => path,
            None => {
                if !rom_path.contains("."){
                    let pos = rom_path.rfind(".").unwrap();
                    format!("{}{}", &rom_path[0..pos], ".rustsav")
                }
                else{
                    format!("{}{}", &rom_path, ".rustsav")
                }
            }
        }; 
        
        let mut save_state = vec![vec![0; 128*1024]; config::NUM_SAVE_STATES];
        // read save path into save_state
        if Path::new(&rom_save_path).exists() {
            let mut reader = BufReader::new(File::open(rom_save_path.clone()).unwrap());
            for i in 0..config::NUM_SAVE_STATES{
                reader.read(&mut save_state[i]).unwrap();
            }
        }

        let initial_save_state = match save_state_bank {
            None => None,
            Some(bank) => Some(save_state[bank-1].as_slice()),
        };

        let res = GBA { 
            bus: Bus::new(rom_path, initial_save_state, cartridge_type_str, apu), 
            cpu: CPU::new(), 
            ppu: PPU::new(), 
            input_handler: InputHandler::new(),
            screenbuf_sender,
            key_receiver,
            fps_sender,

            rom_save_path,
            save_state,
        };

        // zero out input registers (NOTE: handled by BIOS)
        //res.input_handler.process_input(&res.key_receiver, &mut res.bus);

        res
    }

    pub fn start(&mut self) -> Result<(), &'static str> {
        let mut clock: u32 = 0;

        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut last_fps_print_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut frame_counter = 0;
        loop {
            // timer clock
            self.bus.timer_clock();

            if clock & (config::AUDIO_SAMPLE_CLOCKS-1) == 0{
                self.bus.apu_clock();
            }

            // cpu clock
            self.cpu.clock(&mut self.bus);

            // ppu clock and check if frame has completed.
            if let Some(buff) = self.ppu.clock(&mut self.bus){
                if let Err(why) = self.screenbuf_sender.send(buff){
                    println!("   screenbuf sending error: {}", why.to_string());
                }

                // handle input once per frame
                self.input_handler.process_input(&self.key_receiver, &mut self.bus);
                if self.input_handler.cur_speedup_state != self.input_handler.prev_speedup_state{
                    self.bus.apu.extern_audio_enabled = self.input_handler.prev_speedup_state;
                    if !self.input_handler.cur_speedup_state{
                        last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    }
                }
                let mut save_updated = false;
                for i in 0..config::NUM_SAVE_STATES{
                    if self.input_handler.save_requested[i] {
                        self.bus.export_sram(&mut self.save_state[i]);
                        self.input_handler.save_requested[i] = false;
                        save_updated = true;
                    }
                }
                if save_updated {
                    let mut writer = BufWriter::new(File::create(self.rom_save_path.clone()).unwrap());
                    for i in 0..config::NUM_SAVE_STATES{
                        writer.write(&self.save_state[i]).unwrap();
                    }
                    println!("save written to {}", self.rom_save_path);
                }
            }
            
            clock += 1;

            if clock == config::CPU_EXECUTION_INTERVAL_CLOCKS{
                if !self.input_handler.cur_speedup_state{
                    if let Some(t) = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time){
                        let nanos_passed = t.as_nanos() as u64;
                        if config::CPU_EXECUTION_INTERVAL_NS as u64 > nanos_passed {
                            thread::sleep(Duration::from_nanos(config::CPU_EXECUTION_INTERVAL_NS as u64 - nanos_passed as u64));
                        }
                    }
                    
                    //while SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time).unwrap().as_nanos() < config::CPU_EXECUTION_INTERVAL_NS as u128{
                        // polling
                    //}
                    last_finished_time = last_finished_time.checked_add(Duration::from_nanos(config::CPU_EXECUTION_INTERVAL_NS as u64)).unwrap();
                }

                frame_counter += 1;

                if frame_counter == config::FPS_RECORD_INTERVAL
                {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    let since = now.checked_sub(last_fps_print_time).unwrap().as_nanos();
                    if since > 0{
                        let fps = config::FPS_RECORD_INTERVAL as f64 * 1000000000. / since as f64;
                        self.fps_sender.send(fps).unwrap();
                        last_fps_print_time = now;
                        #[cfg(feature="print_cps")]
                        println!("frames per second: {:#.3}", fps);
                    }
                    frame_counter = 0;
                }
                #[cfg(feature="debug_instr")]
                {
                    self.cpu.debug_cnt += 200;
                }

                clock = 0;
            }
        }
    }
}