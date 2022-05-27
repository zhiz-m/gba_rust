use std::{
    env, fs::File, io::{Read, BufReader},
    collections::HashSet,

};
use crate::{
    dma_channel::DMA_Channel,
    fast_hasher::FastHashBuilder,
};

const MEM_MAX: usize = 268435456;

pub struct Bus{
    mem: Box<[u8; MEM_MAX]>,
    cpu_halt_request: bool,
    addr_special_handling: HashSet<usize, FastHashBuilder>,
    
    pub is_any_dma_active: bool,
    pub hblank_dma: bool,
    pub vblank_dma: bool,
    pub dma_channels: [DMA_Channel; 4],
}

impl Bus {
    pub fn new(rom_path : String) -> Bus{
        let mut res = Bus { 
            mem: Box::new([0; MEM_MAX]),
            cpu_halt_request: false,
            addr_special_handling: HashSet::with_hasher(FastHashBuilder),
            
            is_any_dma_active: false,
            hblank_dma: false,
            vblank_dma: false,
            dma_channels: [
                DMA_Channel::new_disabled(0), 
                DMA_Channel::new_disabled(1),
                DMA_Channel::new_disabled(2),
                DMA_Channel::new_disabled(3),
            ]
        };

        // load special addresses
        res.addr_special_handling.insert(0x04000301);
        res.addr_special_handling.insert(0x04000202);
        res.addr_special_handling.insert(0x04000203);
        res.addr_special_handling.insert(0x040000bb);
        res.addr_special_handling.insert(0x040000c7); // + 12
        res.addr_special_handling.insert(0x040000d3); // + 12
        res.addr_special_handling.insert(0x040000df); // + 12

        // load BIOS
        let bios_path = env::var("GBA_RUST_BIOS").unwrap();
        let f = File::open(bios_path).unwrap().bytes();
        for (i, x) in f.enumerate(){
            res.store_byte(i, x.unwrap());
        }

        // load ROM
        let mut reader = BufReader::new(File::open(rom_path).unwrap());
        reader.read(&mut res.mem[0x08000000..]).unwrap();

        // load ROM
        //let f = File::open(rom_path).unwrap().bytes();
        //for (i, x) in f.enumerate(){
        //    res.store_byte(i + 0x08000000, x.unwrap());
        //};

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

    /*#[inline(always)]
    pub fn check_cpu_interrupt(&mut self) -> u16 {
        let res = self.cpu_interrupt;
        self.cpu_interrupt = 0;
        res
    }*/

    pub fn cpu_interrupt(&mut self, interrupt: u16) {
        let reg_if = self.read_halfword(0x04000202);
        let cur_reg_if = interrupt & self.read_halfword(0x04000200);
        self.store_halfword(0x04000202, cur_reg_if & !(reg_if));
    }

    // -------- miscellaneous methods to provide bulk read access. Intended for PPU only with no special functions. 
    //pub fn bulk_read_byte(&self, addr: usize, num: usize) -> &[u8] {
    //    &self.mem[addr .. addr+num]
    //}

    // -------- helper functions
    
    pub fn set_is_any_dma_active(&mut self) {
        self.is_any_dma_active = false;
        for i in 0..4{
            if self.dma_channels[i].is_enabled{
                self.is_any_dma_active = true;
                //println!("dma enabled");
                return;
            }
        }
    }

    fn internal_write_byte(&mut self, addr: usize, val: u8) {
        //if addr == 0x040000bb {
        //    println!("dma channel 0 write");
        //}
        //if self.addr_special_handling.contains(&addr){
        if 0x040000bb <= addr && addr <= 0x04000301{
            match addr{
                0x04000301 => {
                    if val >> 7 > 0 {
                        // todo: add handling for STOP state (pause sound, PPU and cpu)
                    }
                    else{
                        // request that CPU is paused until next interrupt
                        self.cpu_halt_request = true; 
                    }
                },

                // special handling for REG_IF, interrupt handling
                0x04000202 | 0x04000203 => {
                    // current bit 0, incoming bit 0 -> result = 0
                    // current bit 1, incoming bit 1 -> result = 0
                    // current bit 1, incoming bit 0 -> result = 1
                    // current bit 0, incoming bit 1 -> result = 1
                    self.mem[addr] ^= val;
                    return;
                },
                0x040000bb | 0x040000c7 | 0x040000d3 | 0x040000df => {
                    self.mem[addr] = val;
                    let channel_no = (addr - 0x040000bb) / 12;
                    let dma_channel = if val >> 7 > 0{
                        DMA_Channel::new_enabled(channel_no, self)
                    }
                    else{
                        DMA_Channel::new_disabled(channel_no)
                    };
                    self.dma_channels[channel_no] = dma_channel;
                    self.set_is_any_dma_active();
                    //println!("set dma flags");
                    return;
                }
                _ => {},
            }
        }

        self.mem[addr] = val;
    }

    fn addr_mirror(addr: usize) -> usize {
        //if addr >= 0x4000000 && addr < 0x4700000 {
        //    return (addr % 0x0010000) + 0x4000000;
        //}
        if addr >= 0x3FFFF00 && addr < 0x4000000 {
            return (addr % 0x100) + 0x3007F00;
            //if addr == 0x3007ffc{
            //    panic!();
            //}
        }

        addr
    }
}