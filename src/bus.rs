use std::{env, fs::File, io::Read};

const MEM_MAX: usize = 268435456;

pub struct Bus{
    mem: Vec<u8>
}

impl Bus {
    pub fn new(rom_path : String) -> Bus{
        let mut res = Bus { 
            mem: vec![0; 268435456]
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

        // set key registers to 1
        //res.store_halfword(0x04000130, 0b1111111111);

        /*   
        let x:u32 = 0b11100011101000000000101100000001;

        let mut i = 0x08000000;
        while i < 0x0fffffff {    
            let cur = res.read_word(i);
            if cur == x{
                println!("match found, addr = {:#x}", i);
            }
            i+=4;
        }
        */
        res
    }

    pub fn read_byte(&self, addr: usize) -> u8 {
        if addr >= MEM_MAX {
            println!("----- bus.read_byte: out of bounds addr {:#x}", addr);
            return 0;
        }
        self.mem[addr]
    }

    pub fn read_halfword(&self, addr: usize) -> u16 {
        if addr >= MEM_MAX {
            println!("----- bus.read_halfword: out of bounds addr {:#x}", addr);
            return 0;
        }
        assert!(addr & 1 == 0);
        self.mem[addr] as u16 + ((self.mem[addr + 1] as u16) << 8)
    }

    pub fn read_word(&self, addr: usize) -> u32 {
        if addr >= MEM_MAX {
            println!("----- bus.read_word: out of bounds addr {:#x}", addr);
            return 0;
        }
        assert!(addr & 0b11 == 0);
        self.read_word_unaligned(addr)
    }

    pub fn read_word_unaligned(&self, addr: usize) -> u32 {
        if addr >= MEM_MAX {
            println!("----- bus.read_word_unaligned: out of bounds addr {:#x}", addr);
            return 0;
        }
        self.mem[addr] as u32 + ((self.mem[addr + 1] as u32) << 8) + ((self.mem[addr + 2] as u32) << 16) + ((self.mem[addr + 3] as u32) << 24)
    }

    pub fn store_byte(&mut self, addr: usize, val: u8) {
        if addr >= MEM_MAX {
            println!("----- bus.store_byte: out of bounds addr {:#x}", addr);
            return;
        }
        self.mem[addr] = val;
    }

    pub fn store_halfword(&mut self, addr: usize, val: u16) {
        if addr >= MEM_MAX {
            println!("----- bus.store_halfword: out of bounds addr {:#x}", addr);
            return;
        }
        assert!(addr & 1 == 0);
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr + 1] = ((val >> 8) & 0b11111111) as u8;
    }

    pub fn store_word(&mut self, addr: usize, val: u32) {
        if addr >= MEM_MAX {
            println!("----- bus.store_word: out of bounds addr {:#x}", addr);
            return;
        }
        assert!(addr & 0b11 == 0);
        self.store_word_unaligned(addr, val);
    }

    pub fn store_word_unaligned(&mut self, addr: usize, val: u32) {
        if addr >= MEM_MAX {
            println!("----- bus.store_word_unaligned: out of bounds addr {:#x}", addr);
            return;
        }
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr + 1] = ((val >> 8) & 0b11111111) as u8;
        self.mem[addr + 2] = ((val >> 16) & 0b11111111) as u8;
        self.mem[addr + 3] = ((val >> 24) & 0b11111111) as u8;
    }
}