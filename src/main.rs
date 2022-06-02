mod bus;
mod cpu;
mod ppu;
mod frontend;
mod config;
mod input_handler;
mod dma_channel;
mod algorithm;
mod timer;
mod apu;

use bus::Bus;
use cpu::CPU;
use input_handler::KeyInput;
use ppu::{
    PPU, ScreenBuffer
};
use apu::APU;
use frontend::{
    Frontend
};
use input_handler::InputHandler;

use std::{env, thread, time:: {SystemTime, UNIX_EPOCH}, sync::mpsc::{self, Sender, Receiver}};

#[cfg(not(feature="no_limit_cps"))]
use std::time::Duration;

struct GBA {
    bus: Bus,
    cpu: CPU,
    ppu: PPU,
    input_handler: InputHandler,

    screenbuf_sender: Sender<ScreenBuffer>,
    key_receiver: Receiver<(KeyInput,bool)>,
}

impl GBA {
    pub fn new(rom_path: String, cartridge_type_str: Option<String>, screenbuf_sender: Sender<ScreenBuffer>, key_receiver: Receiver<(KeyInput,bool)>, audio_sender: Sender<(f32, f32)>, audio_sample_rate: usize) -> GBA {
        let apu = APU::new(audio_sample_rate, audio_sender);
        let res = GBA { 
            bus: Bus::new(rom_path, cartridge_type_str, apu), 
            cpu: CPU::new(), 
            ppu: PPU::new(), 
            input_handler: InputHandler::new(),
            screenbuf_sender,
            key_receiver,
        };

        // zero out input registers (NOTE: handled by BIOS)
        //res.input_handler.process_input(&res.key_receiver, &mut res.bus);

        res
    }

    pub fn start(&mut self) -> Result<(), &'static str> {
        let mut clock: u64 = 0;

        #[cfg(not(feature="no_limit_cps"))]
        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        
        #[cfg(feature="print_cps")]
        let mut last_clock_print_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        loop {
            if clock & (16 * 1024 * 1024 - 1) == 0{
                #[cfg(feature="print_cps")]
                {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                    let since = now.checked_sub(last_clock_print_time).unwrap().as_millis();
                    if since > 0{
                        let cps = 16. * 1024. * 1024. * 1000. / since as f64;
                        last_clock_print_time = now;
                        println!("clocks per second: {:#.3}", cps);
                    }
                }
                #[cfg(feature="debug_instr")]
                {
                    self.cpu.debug_cnt += 200;
                }
            }

            // apu clock
            if clock & 0b111111111 == 0{
                self.bus.apu_clock();
            }
            
            // timer clock
            self.bus.timer_clock();

            // cpu clock
            self.cpu.clock(&mut self.bus);
            
            // ppu clock and check if frame has completed.
            if let Some(buff) = self.ppu.clock(&mut self.bus){
                if let Err(why) = self.screenbuf_sender.send(buff){
                    println!("   screenbuf sending error: {}", why.to_string());
                }

                // handle input once per frame
                self.input_handler.process_input(&self.key_receiver, &mut self.bus);
            }
            
            clock += 1;

            #[cfg(not(feature="no_limit_cps"))]
            if clock % config::CPU_EXECUTION_INTERVAL_CLOCKS == 0{
                while SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time).unwrap().as_nanos() < config::CPU_EXECUTION_INTERVAL_NS as u128{
                    // polling
                }
                last_finished_time = last_finished_time.checked_add(Duration::from_nanos(config::CPU_EXECUTION_INTERVAL_NS)).unwrap();
            }
            
        }
    }
}

fn main() {
    let rom_path = env::args().nth(1).expect("first argument must be the path to a .gba ROM fle");
    let cartridge_type_str = env::args().nth(2);

    let (tx, rx) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();
    
    // audio
    let (tx3, rx3) = mpsc::channel();

    let mut frontend = Frontend::new("gba_rust frontend".to_string(), rx, tx2, rx3);
    let mut gba = GBA::new(rom_path, cartridge_type_str, tx, rx2, tx3, frontend.get_sample_rate());

    thread::spawn(move || {
        gba.start().unwrap();
    });

    frontend.start().unwrap();
}
