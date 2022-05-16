mod bus;
mod cpu;
mod ppu;
mod frontend;

use bus::Bus;
use cpu::CPU;
use ppu::PPU;
use frontend::{
    Frontend, ScreenBuffer, Pixel
};

use std::{env, fs::File, io::Read, thread, time, sync::mpsc::{self, Sender}};

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

        loop {
            if clock % 10000000 == 0{
                self.cpu.clock(&mut self.bus);
                if let Some(buff) = self.ppu.clock(&mut self.bus){
                    if let Err(why) = self.buff_sender.send(buff){
                        println!("                 buff sending error: {}", why.to_string());
                    }
                }
                //println!("Clock: {}, pc: {:#x}", clock, self.cpu.actual_pc);
                //self.cpu.print_pc();

                //thread::sleep(time::Duration::from_millis(63));
            }/*
            if clock % 1000 == 0{
                println!("Clock: {}", clock);
                self.cpu.print_pc();
            }*/
            clock += 1;
            
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
