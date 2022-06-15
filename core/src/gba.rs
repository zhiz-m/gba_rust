
use crate::{bus::Bus, cpu::CPU, ppu::{PPU, ScreenBuffer}, input_handler::{InputHandler, KeyInput}, apu::APU, config};

use std::{thread, time:: {SystemTime, UNIX_EPOCH, Duration}, sync::mpsc::{Sender, Receiver}, io::{BufReader, Read, BufWriter, Write}, fs::{File, self}, path::Path};

// smaller values have priority. 
#[cfg(feature="binary_heap_loop")]
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
enum Workflow{
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

    screenbuf_sender: Sender<ScreenBuffer>,
    key_receiver: Receiver<(KeyInput,bool)>,
    fps_sender: Sender<f64>,

    rom_save_path: String,
    save_state: Vec<Vec<u8>>,
}

impl GBA {
    pub fn new(bios_path: &str, rom_path: &str, rom_save_path: Option<&str>, save_state_bank: Option<usize>, cartridge_type_str: Option<&str>, screenbuf_sender: Sender<ScreenBuffer>, key_receiver: Receiver<(KeyInput,bool)>, audio_sender: Sender<(f32, f32)>, audio_sample_rate: usize, fps_sender: Sender<f64>) -> GBA {
        let apu = APU::new(audio_sample_rate, audio_sender);

        let rom_save_path = match rom_save_path {
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
        }

        let initial_save_state = match save_state_bank {
            None => None,
            Some(bank) => Some(save_state[bank-1].as_slice()),
        };

        let res = GBA { 
            bus: Bus::new(bios_path, rom_path, initial_save_state, cartridge_type_str, apu), 
            //cpu: CPU::new(), 
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

    #[cfg(not(feature="binary_heap_loop"))]
    pub fn start(&mut self) -> Result<(), &'static str> {
        let mut clock: u32 = 0;

        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut last_fps_print_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut frame_counter = 0;
        loop {
            // timer clock
            if clock & (config::TIMER_CLOCK_INTERVAL_CLOCKS-1) == 0{
                self.bus.timer_clock();
            }

            // cpu clock
            self.bus.cpu_clock();

            if clock & (config::AUDIO_SAMPLE_CLOCKS-1) == 0{
                self.bus.apu_clock();
            }

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
                        self.ppu.frame_count_render = 1;
                    }
                    else{
                        self.ppu.frame_count_render = config::FRAME_RENDER_INTERVAL_SPEEDUP;
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
                    let mut writer = BufWriter::new(File::create(&self.rom_save_path).unwrap());
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
                        //polling
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
                    self.bus.cpu.debug_cnt += 50;
                }

                clock = 0;
            }
        }
    }

    #[cfg(feature="binary_heap_loop")]
    pub fn start(&mut self) -> Result<(), &'static str> {
        use std::{collections::BinaryHeap, cmp::Reverse};

        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut last_fps_print_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        let mut frame_counter = 0;

        let mut heap = BinaryHeap::from(
            [
                Reverse((0, Workflow::Timer)),
                Reverse((0, Workflow::CPU)),
                Reverse((0, Workflow::APU)),
                Reverse((0, Workflow::PPU)),
                Reverse((0, Workflow::Normaliser)),
            ]
        );

        loop {
            let cur = heap.peek().expect("logical error: binary heap is empty").0;
            heap.pop().unwrap();
            //println!("current item in heap: {}, {}", cur.0, cur.1 as u32);
            match cur.1 {
                Workflow::Timer => {
                    self.bus.timer_clock();
                    heap.push(Reverse((cur.0 + config::TIMER_CLOCK_INTERVAL_CLOCKS, Workflow::Timer)));
                },
                Workflow::CPU => {
                    heap.push(Reverse((cur.0 + self.bus.cpu_clock(), Workflow::CPU)));
                },
                Workflow::APU => {
                    self.bus.apu_clock();
                    heap.push(Reverse((cur.0 + config::AUDIO_SAMPLE_CLOCKS, Workflow::APU)));
                },
                Workflow::PPU => {
                    let (clocks, buff) = self.ppu.clock(&mut self.bus);
                    if let Some(buff) = buff{
                        if let Err(why) = self.screenbuf_sender.send(buff){
                            println!("   screenbuf sending error: {}", why.to_string());
                        }
        
                        // handle input once per frame
                        self.input_handler.process_input(&self.key_receiver, &mut self.bus);
                        if self.input_handler.cur_speedup_state != self.input_handler.prev_speedup_state{
                            self.bus.apu.extern_audio_enabled = self.input_handler.prev_speedup_state;
                            if !self.input_handler.cur_speedup_state{
                                last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                                self.ppu.frame_count_render = 1;
                            }
                            else{
                                self.ppu.frame_count_render = config::FRAME_RENDER_INTERVAL_SPEEDUP;
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
                            let mut writer = BufWriter::new(File::create(&self.rom_save_path).unwrap());
                            for i in 0..config::NUM_SAVE_STATES{
                                writer.write(&self.save_state[i]).unwrap();
                            }
                            println!("save written to {}", self.rom_save_path);
                        }
                    }
                    heap.push(Reverse((cur.0 + clocks, Workflow::PPU)));
                },
                Workflow::Normaliser => {
                    if !self.input_handler.cur_speedup_state{
                        if let Some(t) = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time){
                            let nanos_passed = t.as_nanos() as u64;
                            if config::CPU_EXECUTION_INTERVAL_NS as u64 > nanos_passed {
                                thread::sleep(Duration::from_nanos(config::CPU_EXECUTION_INTERVAL_NS as u64 - nanos_passed as u64));
                            }
                        }
                        
                        //while SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time).unwrap().as_nanos() < config::CPU_EXECUTION_INTERVAL_NS as u128{
                            //polling
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
                        self.bus.cpu.debug_cnt += 50;
                    }

                    

                    // roughly every second in real-time, we want to normalize all the values in the heap
                    if cur.0 >= config::CPU_EXECUTION_INTERVAL_CLOCKS * 60{
                        let mut items = vec![];
                        for item in heap.iter(){
                            items.push(Reverse((item.0.0 - cur.0, item.0.1)));
                        }
                        heap.clear();
                        for item in items.into_iter(){
                            heap.push(item);
                        }
                        heap.push(Reverse((config::CPU_EXECUTION_INTERVAL_CLOCKS, Workflow::Normaliser)));
                    }
                    else{
                        heap.push(Reverse((cur.0 + config::CPU_EXECUTION_INTERVAL_CLOCKS, Workflow::Normaliser)));
                    }
                }
            }
        }
    }
}