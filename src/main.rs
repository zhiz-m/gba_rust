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

use std::{env, fs::File, io::Read, thread, time};

struct Emulator {
    bus: Bus,
    cpu: CPU,
    ppu: PPU,

    frontend: Frontend,
}

impl Emulator {
    pub fn new(title: String, rom_path: String) -> Emulator {
        Emulator { 
            bus: Bus::new(rom_path), 
            cpu: CPU::new(), 
            ppu: PPU::new(), 
            frontend: Frontend::new(title),
        }
    }

    pub fn start_loop(&mut self) -> Result<(), &'static str> {
        self.frontend.start()?;

        let mut clock: u64 = 0;

        loop {
            if clock % 10000000 == 0{
                self.cpu.clock(&mut self.bus);
                let res = self.ppu.clock(&mut self.bus);

                if let Some(buf) = res {
                    self.frontend.render(buf)?;
                }

                thread::sleep(time::Duration::from_millis(63));
            }
            clock += 1;
        }

        Ok(())
    }
}

fn main() {
    let rom_path = env::args().nth(1).unwrap();

    let mut emulator = Emulator::new("gba_rust".to_string(), rom_path);

    emulator.start_loop().unwrap();
}
