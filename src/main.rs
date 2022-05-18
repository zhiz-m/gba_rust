mod bus;
mod cpu;
mod ppu;
mod frontend;
mod config;

use bus::Bus;
use cpu::CPU;
use ppu::PPU;
use frontend::{
    Frontend, ScreenBuffer, Pixel
};

use core::time;
use std::{env, thread, time:: {SystemTime, UNIX_EPOCH, Duration}, sync::mpsc::{self, Sender}};

struct Emulator {
    bus: Bus,
    cpu: CPU,
    ppu: PPU,

    buff_sender: Sender<ScreenBuffer>,
}

impl Emulator {
    pub fn new(rom_path: String, buff_sender: Sender<ScreenBuffer>) -> Emulator {
        Emulator { 
            bus: Bus::new(rom_path), 
            cpu: CPU::new(), 
            ppu: PPU::new(), 
            buff_sender
        }
    }

    pub fn start_loop(&mut self) -> Result<(), &'static str> {
        let mut clock: u64 = 0;
        let mut last_finished_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        loop {
            if clock % 16000000 < 100 {
                //println!("  pc: {:#x}, instr: {:#034b}", self.cpu.actual_pc, self.cpu.instr);
                //self.cpu.debug = true;
            }
            else{
                self.cpu.debug = false;
            }
            self.cpu.clock(&mut self.bus);

            /*let addr = 0x6000000 + 240 * 100;
            for i in 0..240{
                self.bus.store_byte(addr + i, 2);
                self.bus.store_byte(addr + 240 + i, 2);
            }

            self.bus.store_halfword(0x5000000, 31);
            */
            /*
            let mut addr = 0x5000000;
            for i in 0..6{
                let res = self.bus.read_halfword(addr + i*2);
                if res > 0 {
                    println!(" i: {}, res: {:#017b}", i, res);
                }
            }
            */
            if let Some(buff) = self.ppu.clock(&mut self.bus){
                if let Err(why) = self.buff_sender.send(buff){
                    println!("                 buff sending error: {}", why.to_string());
                }
            }
            
            if clock % 16000000 == 100 {
                println!();
            }

            clock += 1;

            if clock % config::CPU_EXECUTION_INTERVAL_CLOCKS == 0{
                while SystemTime::now().duration_since(UNIX_EPOCH).unwrap().checked_sub(last_finished_time).unwrap().as_nanos() < config::CPU_EXECUTION_INTERVAL_NS as u128{
                    // polling
                }
                last_finished_time = last_finished_time.checked_add(Duration::from_nanos(config::CPU_EXECUTION_INTERVAL_NS)).unwrap();
            }
            
        }

        Ok(())
    }
}

fn main() {
    let rom_path = env::args().nth(1).unwrap();

    let (tx, rx) = mpsc::channel();

    let mut emulator = Emulator::new(rom_path, tx);
    let mut frontend = Frontend::new("gba_rust frontend".to_string(), rx);

    thread::spawn(move || {
        emulator.start_loop().unwrap();
    });

    frontend.start().unwrap();
}
