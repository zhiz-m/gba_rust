mod bus;
mod cpu;
mod ppu;
mod frontend;
mod config;
mod input_handler;

use bus::Bus;
use cpu::CPU;
use input_handler::KeyInput;
use ppu::{
    PPU, ScreenBuffer
};
use frontend::{
    Frontend
};
use input_handler::InputHandler;

use std::{env, thread, time:: {SystemTime, UNIX_EPOCH, Duration}, sync::mpsc::{self, Sender, Receiver}};

struct Emulator {
    bus: Bus,
    cpu: CPU,
    ppu: PPU,
    input_handler: InputHandler,

    screenbuf_sender: Sender<ScreenBuffer>,
    key_receiver: Receiver<(KeyInput,bool)>,
}

impl Emulator {
    pub fn new(rom_path: String, screenbuf_sender: Sender<ScreenBuffer>, key_receiver: Receiver<(KeyInput,bool)>) -> Emulator {
        let mut res = Emulator { 
            bus: Bus::new(rom_path), 
            cpu: CPU::new(), 
            ppu: PPU::new(), 
            input_handler: InputHandler::new(),
            screenbuf_sender,
            key_receiver,
        };

        // zero out input registers
        res.input_handler.process_input(&res.key_receiver, &mut res.bus);

        res
    }

    pub fn start_loop(&mut self) -> Result<(), &'static str> {
        let mut clock: u64 = 0;
        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        loop {
            
            if clock % 16000000 == 0{
                self.cpu.debug = true;println!();
            }
            else{
                self.cpu.debug = false;
            }
            //self.cpu.debug = true;
            self.cpu.clock(&mut self.bus);
            if let Some(buff) = self.ppu.clock(&mut self.bus){
                if let Err(why) = self.screenbuf_sender.send(buff){
                    println!("   screenbuf sending error: {}", why.to_string());
                }
                self.input_handler.process_input(&self.key_receiver, &mut self.bus);
            }

            clock += 1;

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
    let rom_path = env::args().nth(1).unwrap();

    let (tx, rx) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();


    let mut emulator = Emulator::new(rom_path, tx, rx2);
    let mut frontend = Frontend::new("gba_rust frontend".to_string(), rx, tx2);

    thread::spawn(move || {
        emulator.start_loop().unwrap();
    });

    frontend.start().unwrap();
}
