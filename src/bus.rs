use std::{env, fs::File, io::Read};

const MEM_MAX: usize = 268435456;

pub struct Bus{
    mem: Vec<u8>,
    cpu_halt_request: bool,
    cpu_interrupt: u16,
}

impl Bus {
    pub fn new(rom_path : String) -> Bus{
        let mut res = Bus { 
            mem: vec![0; 268435456],
            cpu_halt_request: false,
            cpu_interrupt: 0,
        };

        // load BIOS
        let bios_path = env::var("GBA_RUST_BIOS").unwrap();
        let f = File::open(bios_path).unwrap().bytes();
        for (i, x) in f.enumerate(){
            res.store_byte(i, x.unwrap());
        }

        // load ROM
        let f = File::open(rom_path).unwrap().bytes();
        for (i, x) in f.enumerate(){
            res.store_byte(i + 0x08000000, x.unwrap());
        };

        res
    }

    // -------- public memory read/write interfaces

    pub fn read_byte(&self, addr: usize) -> u8 {
        if addr >= MEM_MAX {
            println!("----- bus.read_byte: out of bounds addr {:#x}", addr);
            return 0;
        }
        let addr = Bus::addr_mirror(addr);
        self.mem[addr]
    }

    pub fn read_halfword(&self, addr: usize) -> u16 {
        if addr >= MEM_MAX {
            println!("----- bus.read_halfword: out of bounds addr {:#x}", addr);
            return 0;
        }
        let addr = Bus::addr_mirror(addr);
        assert!(addr & 1 == 0);
        self.mem[addr] as u16 + ((self.mem[addr + 1] as u16) << 8)
    }

    pub fn read_word(&self, addr: usize) -> u32 {
        if addr >= MEM_MAX {
            println!("----- bus.read_word: out of bounds addr {:#x}", addr);
            return 0;
        }
        let addr = Bus::addr_mirror(addr);
        assert!(addr & 0b11 == 0);
        self.mem[addr] as u32 + ((self.mem[addr + 1] as u32) << 8) + ((self.mem[addr + 2] as u32) << 16) + ((self.mem[addr + 3] as u32) << 24)
    }

    pub fn store_byte(&mut self, addr: usize, val: u8) {
        if addr >= MEM_MAX {
            println!("----- bus.store_byte: out of bounds addr {:#x}", addr);
            return;
        }
        let addr = Bus::addr_mirror(addr);
        self.internal_write_byte(addr, val);
    }

    pub fn store_halfword(&mut self, addr: usize, val: u16) {
        if addr >= MEM_MAX {
            println!("----- bus.store_halfword: out of bounds addr {:#x}", addr);
            return;
        }
        let addr = Bus::addr_mirror(addr);
        assert!(addr & 1 == 0);
        self.internal_write_byte(addr, (val & 0b11111111) as u8);
        self.internal_write_byte(addr + 1, ((val >> 8) & 0b11111111) as u8);
    }

    pub fn store_word(&mut self, addr: usize, val: u32) {
        if addr >= MEM_MAX {
            println!("----- bus.store_word: out of bounds addr {:#x}", addr);
            return;
        }
        let addr = Bus::addr_mirror(addr);
        assert!(addr & 0b11 == 0);
        self.internal_write_byte(addr, (val & 0b11111111) as u8);
        self.internal_write_byte(addr + 1, ((val >> 8) & 0b11111111) as u8);
        self.internal_write_byte(addr + 2, ((val >> 16) & 0b11111111) as u8);
        self.internal_write_byte(addr + 3, ((val >> 24) & 0b11111111) as u8);
    }

    // -------- miscellaneous public methods to communicate with other components of GBA system
    #[inline(always)]
    pub fn check_cpu_halt_request(&mut self) -> bool {
        if self.cpu_halt_request {
            self.cpu_halt_request = false;
            true
        }
        else{
            false
        }
    }

    #[inline(always)]
    pub fn check_cpu_interrupt(&mut self) -> u16 {
        let res = self.cpu_interrupt;
        self.cpu_interrupt = 0;
        res
    }

    // -------- miscellaneous methods to provide bulk read access. Intended for PPU only with no special functions. 
    //pub fn bulk_read_byte(&self, addr: usize, num: usize) -> &[u8] {
    //    &self.mem[addr .. addr+num]
    //}

    // -------- helper functions

    fn internal_write_byte(&mut self, addr: usize, val: u8) {
        if addr == 0x04000301 {
            if val >> 7 > 0 {
                // todo: add handling for STOP state (pause sound, PPU and cpu)
            }
            else{
                // request that CPU is paused until next interrupt
                self.cpu_halt_request = true; 
            }
        }

        // special handling for REG_IF, interrupt handling
        else if addr == 0x04000202 || addr == 0x04000203 {
            // current bit 0, incoming bit 0 -> result = 0
            // current bit 1, incoming bit 1 -> result = 0
            // current bit 1, incoming bit 0 -> result = 1
            // current bit 0, incoming bit 1 -> result = 1
            self.mem[addr] ^= val;
            return;
        }

        self.mem[addr] = val;
    }

    fn addr_mirror(addr: usize) -> usize {
        if addr >= 0x4000000 && addr < 0x4700000 {
            return (addr % 0x0010000) + 0x4000000;
        }
        if addr >= 0x3000000 && addr < 0x4000000 {
            let addr = (addr % 0x8000 ) + 0x3000000;
            //if addr == 0x3007ffc{
            //    panic!();
            //}
            return addr;
        }

        addr
    }
}