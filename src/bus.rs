use std::{
    env, fs::File, io::{Read, BufReader},
    collections::HashSet,

};
use crate::{
    dma_channel::DMA_Channel,
    algorithm::{
        FastHashBuilder,
        self,
    },
    timer::Timer,
    config,
    apu::APU,
};

const MEM_MAX: usize = 268435456;

#[derive(Clone, Copy, PartialEq)]
pub enum ChunkSize{
    Word = 4,
    Halfword = 2,
    Byte = 1,
}

#[derive(Clone, Copy)]
enum MemoryRegion{
    BIOS,
    BoardWRAM,
    ChipWRAM,
    IO,
    Palette,
    VRAM,
    OAM,
    Cartridge,
    CartridgeSRAM,
    Illegal,
}

#[derive(Clone, Copy)]
pub enum CartridgeType{
    EEPROM,
    SRAM,
    FLASH64,
    FLASH128,
}

fn derive_cartridge_type(cartridge: &[u8]) -> CartridgeType {
    let matches = [
        "SRAM_V".as_bytes(), 
        "FLASH_V".as_bytes(), 
        "FLASH512_V".as_bytes(), 
        "FLASH1M_V".as_bytes(),
        "EEPROM_V".as_bytes(), 
    ];
    let res = algorithm::u8_search(cartridge, &matches);
    match res {
        None => config::DEFAULT_CARTRIDGE_TYPE,
        Some(res) => {
            match res {
                0 => CartridgeType::SRAM,
                1 | 2 => CartridgeType::FLASH64,
                3 => CartridgeType::FLASH128,
                4 => CartridgeType::EEPROM,
                _ => panic!("logical error, invalid result from u8_search"),
            }
        }
    }
}

pub struct Bus{
    mem: Vec<u8>,
    cpu_halt_request: bool,
    addr_special_handling: HashSet<usize, FastHashBuilder>,

    cartridge_type: CartridgeType,

    // 0-2: cartridge command flags
    // 3: cartridge page number (for 218kb only, 0 or 1)
    // 4: cartridge mode 
    //     val=0 read mode
    //     val=1 device&manufacturer info mode
    //     val=2: erase mode
    //     val=3: write single byte mode
    //     val=4: select page number mode
    cartridge_type_state: [u8; 7], 

    pub is_any_dma_active: bool,
    pub hblank_dma: bool,
    pub vblank_dma: bool,
    pub dma_channels: [DMA_Channel; 4],

    pub is_any_timer_active: bool,
    timers: [Timer; 4],

    apu: APU,
}

impl Bus {
    pub fn new(rom_path : String, cartridge_type_str: Option<String>, apu: APU) -> Bus{
        let mut mem = vec![0; MEM_MAX];

        // load BIOS
        let bios_path = env::var("GBA_RUST_BIOS").unwrap();
        let mut reader = BufReader::new(File::open(bios_path).unwrap());
        reader.read(&mut mem[0..]).unwrap();

        // load ROM
        let mut reader = BufReader::new(File::open(rom_path).unwrap());
        reader.read(&mut mem[0x08000000..]).unwrap();

        let cartridge_type = match cartridge_type_str {
            None => derive_cartridge_type(&mem[0x08000000..]),
            Some(cartridge_type_str) => {
                let cartridge_type_str = cartridge_type_str.trim().to_ascii_uppercase();
                let trimmed_str = cartridge_type_str.split(" ").nth(0).unwrap();
                match trimmed_str {
                    "SRAM" => CartridgeType::SRAM,
                    "FLASH" => CartridgeType::FLASH64,
                    "FLASH512" => CartridgeType::FLASH64,
                    "FLASH1M" => CartridgeType::FLASH128,
                    "EEPROM" => CartridgeType::FLASH128,
                    _ => panic!()
                }
            }
        };

        println!("backup type: {}", cartridge_type as u32);

        let mut res = Bus { 
            mem,
            cpu_halt_request: false,
            addr_special_handling: HashSet::with_hasher(FastHashBuilder),
            cartridge_type,
            cartridge_type_state: [0; 7],
            
            is_any_dma_active: false,
            hblank_dma: false,
            vblank_dma: false,
            dma_channels: [
                DMA_Channel::new_disabled(0), 
                DMA_Channel::new_disabled(1),
                DMA_Channel::new_disabled(2),
                DMA_Channel::new_disabled(3),
            ],

            is_any_timer_active: false,
            timers: [
                Timer::new(0),
                Timer::new(1),
                Timer::new(2),
                Timer::new(3),
            ],

            apu,
        };

        // load special addresses
        res.addr_special_handling.insert(0x04000301);
        res.addr_special_handling.insert(0x04000202);
        res.addr_special_handling.insert(0x04000203);
        res.addr_special_handling.insert(0x040000bb);
        res.addr_special_handling.insert(0x040000c7); // + 12
        res.addr_special_handling.insert(0x040000d3); // + 12
        res.addr_special_handling.insert(0x040000df); // + 12

        res
    }

    // -------- public memory read/write interfaces, intended for user instructions. 

    pub fn read_byte(&self, addr: usize) -> u8 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Byte, true);
        self.internal_read_byte(addr, region)
    }

    pub fn read_halfword(&self, addr: usize) -> u16 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Halfword, true);
        assert!(addr & 1 == 0);
        self.internal_read_byte(addr, region) as u16 + ((self.internal_read_byte(addr + 1, region) as u16) << 8)
    }

    pub fn read_word(&self, addr: usize) -> u32 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Word, true);
        assert!(addr & 0b11 == 0);
        self.internal_read_byte(addr, region) as u32 + ((self.internal_read_byte(addr+1, region) as u32) << 8) + ((self.internal_read_byte(addr+2, region) as u32) << 16) + ((self.internal_read_byte(addr+3, region) as u32) << 24)
    }

    pub fn store_byte(&mut self, addr: usize, val: u8) {
        let (addr, region) = self.addr_match(addr, ChunkSize::Byte, false);
        self.internal_write_byte(addr, region, val);
    }

    pub fn store_halfword(&mut self, addr: usize, val: u16) {
        let (addr, region) = self.addr_match(addr, ChunkSize::Halfword, false);
        assert!(addr & 1 == 0);
        self.internal_write_byte(addr, region, (val & 0b11111111) as u8);
        self.internal_write_byte(addr + 1, region, ((val >> 8) & 0b11111111) as u8);
    }

    pub fn store_word(&mut self, addr: usize, val: u32) {
        let (addr, region) = self.addr_match(addr, ChunkSize::Word, false);
        assert!(addr & 0b11 == 0);
        self.internal_write_byte(addr, region, (val & 0b11111111) as u8);
        self.internal_write_byte(addr + 1, region, ((val >> 8) & 0b11111111) as u8);
        self.internal_write_byte(addr + 2, region, ((val >> 16) & 0b11111111) as u8);
        self.internal_write_byte(addr + 3, region, ((val >> 24) & 0b11111111) as u8);
    }

    // -------- fast read/write interfaces, intended for use by system (not user instructions)
    //          note: these functions do not perform any wrapping at all. 

    pub fn read_byte_raw(&self, addr: usize) -> u8 {
        self.mem[addr]
    }

    pub fn read_halfword_raw(&self, addr: usize) -> u16 {
        self.mem[addr] as u16 + ((self.mem[addr + 1] as u16) << 8)
    }

    pub fn read_word_raw(&self, addr: usize) -> u32 {
        self.mem[addr] as u32 + ((self.mem[addr + 1] as u32) << 8) + ((self.mem[addr + 2] as u32) << 16) + ((self.mem[addr + 3] as u32) << 24)
    }

    pub fn store_byte_raw(&mut self, addr: usize, val: u8) {
        self.mem[addr] = val;
    }

    pub fn store_halfword_raw(&mut self, addr: usize, val: u16) {
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr+1] = ((val >> 8) & 0b11111111) as u8;
    }

    pub fn store_word_raw(&mut self, addr: usize, val: u32) {
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr+1] = ((val >> 8) & 0b11111111) as u8;
        self.mem[addr+2] = ((val >> 16) & 0b11111111) as u8;
        self.mem[addr+3] = ((val >> 24) & 0b11111111) as u8;
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

    pub fn cpu_interrupt(&mut self, interrupt: u16) {
        let reg_if = self.read_halfword(0x04000202);
        let cur_reg_if = interrupt & self.read_halfword(0x04000200);
        self.store_halfword(0x04000202, cur_reg_if & !(reg_if));
    }

    pub fn timer_clock(&mut self) {
        if !self.is_any_timer_active{
            return;
        }
        //println!("timer clock");
        unsafe{
            for i in 0..4 {
                let ptr = &mut self.timers[i] as *mut Timer;
                if (*ptr).is_enabled && (*ptr).clock(self) && i != 3 && self.timers[i+1].is_cascading{
                    let ptr = &mut self.timers[i+1] as *mut Timer;
                    (*ptr).cascade();
                }
            }
        }
    }

    // note: for clarify, channels 1-4 will be representing using numbers 0-3
    pub fn apu_clock(&mut self) {
        let ptr = &mut self.apu as *mut APU;
        unsafe {
            (*ptr).clock_512(self);
        }
    }

    // -------- helper functions
    pub fn set_is_any_dma_active(&mut self) {
        self.is_any_dma_active = false;
        for i in 0..4{
            if self.dma_channels[i].is_enabled{
                self.is_any_dma_active = true;
                return;
            }
        }
    }

    pub fn set_is_any_timer_active(&mut self) {
        self.is_any_timer_active = false;
        for i in 0..4{
            if self.timers[i].is_enabled{
                self.is_any_timer_active = true;
                return;
            }
        }
    }

    fn internal_read_byte(&self, addr: usize, region: MemoryRegion) -> u8 {
        match region {
            MemoryRegion::IO => {
                if 0x4000100 <= addr && addr <= 0x400010e {
                    match addr {
                        0x4000100 => self.timers[0].timer_count as u8,
                        0x4000101 => (self.timers[0].timer_count >> 8) as u8,
                        0x4000104 => self.timers[1].timer_count as u8,
                        0x4000105 => (self.timers[1].timer_count >> 8) as u8,
                        0x4000108 => self.timers[2].timer_count as u8,
                        0x4000109 => (self.timers[2].timer_count >> 8) as u8,
                        0x400010c => self.timers[3].timer_count as u8,
                        0x400010d => (self.timers[3].timer_count >> 8) as u8,
                        _ => self.mem[addr],
                    }
                }
                else{
                    self.mem[addr]
                }
            }
            MemoryRegion::CartridgeSRAM => {
                //println!("read from SRAM, addr: {:#x}, val: {:#x}", addr, self.mem[addr]);
                match self.cartridge_type {
                    CartridgeType::SRAM => self.mem[addr],
                    CartridgeType::FLASH64 | CartridgeType::FLASH128 => self.internal_read_byte_flash(addr),
                    _ => panic!("writing to SRAM is forbidden for cartridge type {}", self.cartridge_type as u32)
                }
                
            },
            MemoryRegion::Illegal => {
                0
            }
            _ => self.mem[addr]
        }
    }

    fn internal_write_byte(&mut self, addr: usize, region: MemoryRegion, val: u8) {
        match region {
            MemoryRegion::IO => {
                if 0x04000065 <= addr && addr <= 0x04000301{
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

                        // special handling for DMA
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
                        },

                        // special handling for writing to timer count
                        0x4000100 | 0x4000101 | 0x4000104 | 0x4000105 | 
                        0x4000108 | 0x4000109 | 0x400010c | 0x400010d => {
                            let timer_no = (addr - 0x4000100) >> 2;
                            unsafe{
                                let ptr = &mut self.timers[timer_no] as *mut Timer;
                                if addr & 1 == 0 {
                                    (*ptr).reload_val &= !0b11111111;
                                    (*ptr).reload_val |= val as u16;
                                }
                                else{
                                    (*ptr).reload_val &= 0b11111111;
                                    (*ptr).reload_val |= (val as u16) << 8;
                                }
                            }
                        },

                        // special handling for timer control
                        0x4000102 | 0x4000106 | 0x400010a | 0x400010e => {
                            let timer_no = (addr - 0x4000102) >> 2;
                            unsafe{
                                let ptr = &mut self.timers[timer_no] as *mut Timer;
                                (*ptr).set_frequency(val & 0b11);
                                (*ptr).is_cascading = (val >> 2) & 1 > 0;
                                (*ptr).raise_interrupt = (val >> 6) & 1 > 0;
                                (*ptr).set_is_enabled((val >> 7) & 1 > 0);
                                self.set_is_any_timer_active();
                                //if (*ptr).is_enabled {
                                //    println!("timer {} enabled", timer_no);
                                //}
                            }
                        },

                        // special handling for square sound channels; reset
                        0x4000065 | 0x400006d => {
                            self.mem[addr] = val;
                            let square_chan_num = match addr {
                                0x4000065 => 0,
                                0x400006d => 1,
                                _ => unreachable!()
                            };
                            if (val >> 7) & 1 > 0 {
                                let ptr = &mut self.apu as * mut APU;
                                unsafe{
                                    (*ptr).reset_square_channel(square_chan_num, self);
                                };
                            }
                            return;
                        }

                        // special handling for enabling sound channels 0 - 3
                        /*0x04000081 => {
                            // i: 1 is left, i: 0 is right
                            for i in 0..2{
                                for j in 0..2 {
                                    if (self.mem[0x04000081] >> (4 - 4*i + j)) & 1 == 0 && (val >> (4 - 4*i + j)) & 1 == 1 {

                                    }
                                }
                            }
                        }*/
                        _ => {},
                    }
                }
                self.mem[addr] = val;
            },
            MemoryRegion::BIOS => {
                // do nothing, writing to BIOS is illegal
            },
            MemoryRegion::CartridgeSRAM => {
                //println!("write to SRAM, addr: {:#x}, val: {:#x}", addr, val);
                match self.cartridge_type {
                    CartridgeType::FLASH64 | CartridgeType::FLASH128 => {
                        self.internal_write_byte_flash(addr, val);
                    },
                    _ => {},
                }
            },
            MemoryRegion::Illegal => {

            },
            _ => {
                self.mem[addr] = val;
            }
        };
    }

    fn internal_read_byte_flash(&self, addr: usize) -> u8 {
        match self.cartridge_type_state[4] {
            //0 => {
                
            //},
            1 => {
                let (device, man) = 
                match self.cartridge_type{
                    CartridgeType::FLASH64 => {
                        (0x1c, 0xc2) // Macronix 64kb
                    },
                    CartridgeType::FLASH128 => {
                        (0x09, 0xc2) // Macronix 128kb
                    },
                    _ => panic!("cartridge type is not flash")
                };
                match addr {
                    0xe000000 => man,
                    0xe000001 => device,
                    _ => panic!("invalid addr for read in device/manufacturer mode"),
                }
            },
            _ => {
                //panic!("invalid cartridge type state for read: {}", self.cartridge_type_state[4])
                match self.cartridge_type{
                    CartridgeType::FLASH64 => {
                        self.mem[config::FLASH64_MEM_START + (addr & 0xffff)]
                    },
                    CartridgeType::FLASH128 => {
                        self.mem[config::FLASH128_MEM_START + (addr & 0xffff) + ((self.cartridge_type_state[3] as usize) << 16)]
                    },
                    _ => panic!("cartridge type is not flash")
                }
            }
        }
    }

    fn internal_write_byte_flash(&mut self, addr: usize, val: u8) {
        match self.cartridge_type_state[4] {
            // write single byte
            3 => {
                match self.cartridge_type{
                    CartridgeType::FLASH64 => {
                        self.mem[config::FLASH64_MEM_START + (addr & 0xffff)] = val;
                    },
                    CartridgeType::FLASH128 => {
                        self.mem[config::FLASH128_MEM_START + (addr & 0xffff) + ((self.cartridge_type_state[3] as usize) << 16)] = val;
                    },
                    _ => panic!("cartridge type is not flash")
                }
                self.cartridge_type_state[4] = 0;
            },
            _ => {
                if addr == 0x0e005555 {
                    if val == 0xaa{
                        self.cartridge_type_state[0] = val;
                        self.cartridge_type_state[2] = 0;
                    }
                    else if self.cartridge_type_state[1] != 0{
                        self.cartridge_type_state[2] = val;
                        self.execute_flash_storage_command();
                    }
                }
                else if addr == 0x0e002aaa {
                    self.cartridge_type_state[1] = val;
                }
                else{
                    match self.cartridge_type_state[4]{
                        2 => {
                            if addr & 0xfff == 0 && self.cartridge_type_state[0] != 0 && self.cartridge_type_state[1] != 0 && val == 0x30 {
                                // special: erase entire sector
                                println!("sector erase: {:#x}", addr);
                                let addr = 
                                match self.cartridge_type{
                                    CartridgeType::FLASH64 => {
                                        config::FLASH64_MEM_START + (addr & 0xffff)
                                    },
                                    CartridgeType::FLASH128 => {
                                        config::FLASH128_MEM_START + (addr & 0xffff) + ((self.cartridge_type_state[3] as usize) << 16)
                                    },
                                    _ => panic!("cartridge type is not flash")
                                };
                                for i in addr..addr+0x1000{
                                    self.mem[i] = 0xff;
                                }
                                self.cartridge_type_state[0] = 0;
                                self.cartridge_type_state[1] = 0;
                                self.cartridge_type_state[2] = 0;
                                self.cartridge_type_state[4] = 0;
                            }
                        }
                        // bank switching
                        4 => {
                            assert!(addr == 0x0e000000);
                            if addr == 0x0e000000{
                                assert!(val <= 1 );
                                self.cartridge_type_state[3] = val;
                                self.cartridge_type_state[4] = 0;
                            }
                        }
                        _ => panic!("invalid cartridge type state for write: {}", self.cartridge_type_state[4])
                    }
                }
            }
        }
    }

    fn execute_flash_storage_command(&mut self) {
        match self.cartridge_type_state[2] {
            0x90 => {
                self.cartridge_type_state[4] = 1;
            },
            0xf0 => {
                if self.cartridge_type_state[4] == 1{
                    self.cartridge_type_state[4] = 0;
                }
            },
            0x80 => {
                self.cartridge_type_state[4] = 2;
            },
            0x10 => {
                if self.cartridge_type_state[4] == 1 {
                    let (start, end) = 
                    match self.cartridge_type{
                        CartridgeType::FLASH64 => (config::FLASH64_MEM_START, config::FLASH64_MEM_END),
                        CartridgeType::FLASH128 => (config::FLASH128_MEM_START, config::FLASH128_MEM_END),
                        _ => panic!("logical error: execute_flash_storage_command is caled, but cartridge type is not FLASH64 or FLASH128"),
                    };
                    for i in start..end {
                        self.mem[i] = 0xff;
                    }
                }
                self.cartridge_type_state[4] = 0;
            },
            0xa0 => {
                self.cartridge_type_state[4] = 3;
            },
            0xb0 => {
                self.cartridge_type_state[4] = 4;
            },
            _ => {}
        }
        self.cartridge_type_state[0] = 0;
        self.cartridge_type_state[1] = 0;
        self.cartridge_type_state[2] = 0;
    }

    fn addr_match(&self, addr: usize, chunk_size: ChunkSize, is_read: bool) -> (usize, MemoryRegion) {
        //if addr >= 0x4000000 && addr < 0x4700000 {
        //    return (addr % 0x0010000) + 0x4000000;
        //}
        match addr >> 24 {
            0 | 1 => {
                if addr >= 0x4000 {
                    println!("illegal memory address: {:#x}", addr);
                    (0, MemoryRegion::Illegal)
                }
                else{
                    (addr, MemoryRegion::BIOS)
                }
            },
            2 => {
                (0x02000000 + (addr & 0x3ffff), MemoryRegion::BoardWRAM)
            },
            3 => {
                (0x03000000 + (addr & 0x7fff), MemoryRegion::ChipWRAM)
            },
            4 => {
                // NOTE: not mirrored (maybe todo)
                (addr, MemoryRegion::IO)
            },
            5 => {
                if !is_read {
                    if let ChunkSize::Byte = chunk_size{
                        return (0, MemoryRegion::Illegal)
                    }
                }
                (0x05000000 + (addr & 0x3ff), MemoryRegion::Palette)
            },
            6 => {
                if !is_read {
                    if let ChunkSize::Byte = chunk_size{
                        return (0, MemoryRegion::Illegal)
                    }
                }
                // NOTE: not mirrored (maybe todo)
                let mut m = addr & 0x1ffff;
                if m >= 98304{
                    m -= 32768;
                }
                (0x06000000 + m, MemoryRegion::VRAM)
            },
            7 => {
                if !is_read {
                    if let ChunkSize::Byte = chunk_size{
                        return (0, MemoryRegion::Illegal)
                    }
                }
                (0x07000000 + (addr & 0x3ff), MemoryRegion::OAM)
            },
            8 | 9 | 10 | 11 | 12 | 13 => {
                if !is_read {
                    return (0, MemoryRegion::Illegal)
                }
                //(addr, MemoryRegion::Cartridge)
                (0x08000000 + (addr & 0x1ffffff), MemoryRegion::Cartridge)
            }
            14 | 15 => {
                /*match self.cartridge_type{
                    CartridgeType::FLASH64 | CartridgeType::FLASH128 => {
                        if !is_read {
                            if let ChunkSize::Byte = chunk_size{
                                return (0, MemoryRegion::Illegal)
                            }
                        }
                    }
                    CartridgeType::SRAM => {
                        if !is_read {
                            if ChunkSize::Byte != chunk_size{
                                return (0, MemoryRegion::Illegal)
                            }
                        }
                    }
                    _ => {},
                }*/
                (0x0e000000 + (addr & 0xffff), MemoryRegion::CartridgeSRAM)
            }
            _ => {
                println!("illegal memory access: > 0x10000000: {:#x}", addr);
                (0, MemoryRegion::Illegal)
            },
        }
    }
}