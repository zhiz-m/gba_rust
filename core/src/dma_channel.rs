#![allow(non_camel_case_types)]

use crate::bus::{Bus, ChunkSize, MemoryRegion, CartridgeType};

#[derive(Clone, Copy, PartialEq)]
pub enum TimingMode {
    Immediate,
    VBlank,
    HBlank,
    FIFO,
}

#[derive(Clone)]
pub struct DMA_Channel<const IS_ARM9: bool> {
    channel_no: usize,
    pub src_addr: usize,
    pub dest_addr: usize,
    src_increment: usize,  // -1, 0, 1.
    dest_increment: usize, // -1, 0, 1.
    num_transfers: u16,
    chunk_size: ChunkSize,
    pub timing_mode: TimingMode,
    raise_interrupt: bool,
    is_repeating: bool,
    repeat_reset_dest: bool,
    pub is_enabled: bool,
}

impl<const IS_ARM9: bool> DMA_Channel<IS_ARM9> {
    pub fn new_disabled(channel_no: usize) -> DMA_Channel<IS_ARM9> {
        DMA_Channel {
            channel_no,
            src_addr: 0,
            dest_addr: 0,
            src_increment: 0,
            dest_increment: 0,
            num_transfers: 0,
            chunk_size: ChunkSize::Word,
            timing_mode: TimingMode::Immediate,
            raise_interrupt: false,
            is_repeating: false,
            repeat_reset_dest: false,
            is_enabled: false,
        }
    }

    pub fn new_enabled(channel_no: usize, bus: &mut Bus) -> DMA_Channel<IS_ARM9> {
        let src_addr = bus.read_word_raw(0xb0 + 12 * channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io}) as usize;
        let dest_addr = bus.read_word_raw(0xb4 + 12 * channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io}) as usize;
        let dma_cnt = bus.read_word_raw(0xb8 + 12 * channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io});
        let mut num_transfers = dma_cnt as u16;
        let timing_bits = if IS_ARM9 {dma_cnt >> 0x1b} else {dma_cnt >> 0x1c} & 0b11;
        let mut is_enabled = true;
        let timing_mode = match timing_bits {
            0b00 => TimingMode::Immediate,
            0b01 => TimingMode::VBlank,
            0b10 => TimingMode::HBlank,
            0b11 => {
                // turn dma channel off, fifo not applicable to nds
                is_enabled = false;
                //let mut dma_cnt_upper = bus.read_byte_raw(0x040000bb + 12 * channel_no);
                //dma_cnt_upper &= !(1 << 7);
                //bus.store_byte_raw(0x040000bb + 12 * channel_no, dma_cnt_upper);
                // assert!(dest_addr == 0x040000a0 || dest_addr == 0x040000a4);
                //println!("dma fifo addr: {:#x}", src_addr)
                // num_transfers = 4;
                TimingMode::FIFO
            }
            _ => unreachable!(),
        };
        if timing_mode == TimingMode::FIFO {
            //println!("dma channel {}, src_addr: {:#x}, dest addr: {:#x}, num_transfers: {:#x}", channel_no, src_addr, dest_addr, dma_cnt as u16);
        }
        //assert!(!is_enabled || dma_cnt as u16 > 0);
        DMA_Channel {
            channel_no,
            src_addr,
            dest_addr,
            src_increment: 0,
            dest_increment: 0,
            num_transfers,
            chunk_size: ChunkSize::Word,
            timing_mode,
            raise_interrupt: false,

            // set here so we know at try_execute_dma whether this is a repeat DMA run
            is_repeating: false,

            repeat_reset_dest: false,
            is_enabled,
        }
    }

    pub fn check_is_active(&self, bus: &Bus) -> bool {
        match self.is_enabled {
            false => false,
            true => {
                //self.is_enabled = bus.read_byte(0x040000bb + 12 * self.channel_no) & 1 > 0;
                //if !self.is_enabled {
                //    false
                //}
                //else{
                match self.timing_mode {
                    TimingMode::Immediate => true,
                    TimingMode::HBlank => bus.hblank_dma,
                    TimingMode::VBlank => bus.vblank_dma,
                    TimingMode::FIFO => {
                        match self.channel_no {
                            0 => {
                                println!("FIFO channel is invalid for DMA channel_no of 0");
                                false
                            }
                            // sound FIFO mode
                            1 | 2 => {
                                bus.apu.direct_sound_fifo[(self.dest_addr - 0x040000a0) >> 2].len()
                                    <= 16
                            }
                            // video transfer mode
                            3 => {
                                bus.hblank_dma && {
                                    let vcount = bus.read_byte_raw(0x5, MemoryRegion::Arm7Io);
                                    vcount >= 2 && vcount < 162
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                //}
            }
        }
    }

    // returns number of cycles
    pub fn execute_dma(&mut self, bus: &mut Bus) -> u32 {
        //self.src_addr = bus.read_word(0x040000b0 + 12 * self.channel_no) as usize;
        //self.dest_addr = bus.read_word(0x040000b4 + 12 * self.channel_no) as usize;
        //if !self.check_is_active(bus){
        //    return 0;
        //}
        let dma_cnt = bus.read_word_raw(0xb8 + 12 * self.channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io});

        if self.is_repeating {
            // if this is a repeat run, need to re-load the number of transfers
            self.num_transfers = match self.timing_mode {
                TimingMode::FIFO => 4,
                _ => dma_cnt as u16,
            };
            if self.repeat_reset_dest {
                self.dest_addr =
                    bus.read_word_raw(0xb4 + 12 * self.channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io}) as usize;
            }
        }

        //self.num_transfers = dma_cnt as u16;
        self.repeat_reset_dest = false;

        self.dest_increment = match self.timing_mode {
            TimingMode::FIFO => 0,
            _ => match (dma_cnt >> 0x15) & 0b11 {
                0b00 => 1,
                0b01 => !0, // -1
                0b10 => 0,
                0b11 => {
                    self.repeat_reset_dest = true;
                    1
                }
                _ => unreachable!(),
            },
        };

        self.src_increment = match (dma_cnt >> 0x17) & 0b11 {
            0b00 => 1,
            0b01 => !0, // -1
            0b10 => 0,
            0b11 => {
                println!("illegal DMA channel src_increment of 0b11");
                0
            }
            _ => unreachable!(),
        };

        if self.timing_mode != TimingMode::FIFO {
            //println!("non-fifo dma dest_addr: {:#x}", self.dest_addr);
            self.chunk_size = match (dma_cnt >> 0x1a) & 1 > 0 {
                true => ChunkSize::Word,
                false => ChunkSize::Halfword,
            };
        } else if self.channel_no == 1 || self.channel_no == 2 {
            assert!(self.chunk_size == ChunkSize::Word);
            assert!(self.num_transfers == 4);
            assert!(self.dest_addr == 0x040000a0 || self.dest_addr == 0x040000a4);
            assert!(self.check_is_active(bus));
        }
        else{
            panic!("video transfer DMA not implemented");
        }

        self.raise_interrupt = (dma_cnt >> 0x1e) & 1 > 0;

        self.is_repeating = self.timing_mode == TimingMode::FIFO
            || (self.timing_mode != TimingMode::Immediate && (dma_cnt >> 0x19) & 1 > 0);

        if self.channel_no != 1 && self.channel_no != 2 {
            //println!("dest: {:#x}, channel_no: {}", self.dest_addr, self.channel_no);
        }
        if self.channel_no == 3{
            //println!("dma channel 3, src addr: {:#x}, dest addr: {:#x}", self.src_addr, self.dest_addr);
        }

        // if num_transfers is 0, set to max size
        if self.num_transfers == 0 {
            self.num_transfers = if IS_ARM9{0x200000} else if self.channel_no == 3 {0x10000} else {0x4000}
        }

        //
        if self.channel_no == 3 && ((self.src_addr >= 0xd000000 && self.src_addr <= 0xdffffff ) || (self.dest_addr >= 0xd000000 && self.dest_addr <= 0xdffffff )) 
            && (bus.cartridge_type == CartridgeType::Eeprom512 || bus.cartridge_type == CartridgeType::Eeprom8192){
            //println!("chunksize: {}, src_inc: {}, dest_inc: {}", self.chunk_size as u32, self.src_increment as i32, self.dest_increment as i32);
            if self.chunk_size == ChunkSize::Halfword && self.src_increment == 1 && self.dest_increment == 1 {
                //if self.dest_addr >= 0xd000000 && self.dest_addr <= 0xdffffff && self.src_addr >= 0xd000000 && self.src_addr <= 0xdffffff{
                //    println!("eeprom src and dest both in eeprom region");
                //}
                //println!("dma num transfers: {}", self.num_transfers);
                // EEPROM write
                if self.dest_addr >= 0xd000000 && self.dest_addr <= 0xdffffff {
                    bus.eeprom_is_read = false;
                    let mut res: u64 = 0;
                    let mut sram_addr = 0;
                    let mut j = 0;
                    let mut is_read = false;
                    for i in 0..self.num_transfers{
                        let data = bus.read_halfword(self.src_addr);
                        res <<= 1;
                        res |= data as u64 & 1;
                        j += 1;
                        if i == 1 {
                            if res == 0b10 {
                                is_read = false;
                                bus.eeprom_is_read = false;
                                //println!("eeprom write");
                            }
                            else if res == 0b11 {
                                is_read = true;
                                bus.eeprom_is_read = true;
                                //println!("eeprom read set addr");
                            } 
                            else{
                                println!("DMA channel 3 EEPROM no matching bits, res: {:#05b}", res);
                                break;
                            }
                            j = 0;
                            res = 0;
                        }
                        else if (i == 7 && bus.cartridge_type == CartridgeType::Eeprom512) || (i == 15 && bus.cartridge_type == CartridgeType::Eeprom8192){
                            //assert!(res < 0x400);
                            sram_addr = res << 3;
                            j = 0;
                            res = 0;
                            if is_read {
                                //println!("EEPROM setting read mem addr");
                                bus.eeprom_read_offset = sram_addr as usize;
                            }
                        }
                        else if !is_read && j == 64{
                            //println!("EEPROM writing to memory, i: {}, num_transfers: {}", i, self.num_transfers);
                            j = 0;
                            let base_addr = sram_addr;
                            bus.store_word_raw(base_addr as usize, MemoryRegion::CartridgeSram, res as u32);
                            bus.store_word_raw(base_addr as usize + 4, MemoryRegion::CartridgeSram, (res >> 32) as u32);
                            //println!("write res: {:#18x}", res);
                            //println!("write base addr: {:#x}", base_addr);
                            bus.eeprom_write_successful = true;
                        }

                        self.src_addr += 2;
                        self.dest_addr += 2;
                    }  
                }
                // EEPROM read
                else if bus.eeprom_is_read{
                    //println!("eeprom read");
                    //bus.eeprom_is_read = false;
                    let mut j = 0;
                    let base_addr = bus.eeprom_read_offset;
                    //println!("read base addr: {:#x}", base_addr);
                    let res = bus.read_word_raw(base_addr as usize, MemoryRegion::CartridgeSram) as u64 +
                        ((bus.read_word_raw(base_addr as usize + 4, MemoryRegion::CartridgeSram) as u64) << 32);
                    //println!("read res: {:#18x}", res);
                    for i in 0..self.num_transfers{
                        j += 1;
                        let mut data = 0;
                        if i == 3 {
                            j = 0;
                        }
                        else if i > 3 && j <= 64 {
                            //println!("j: {}", j);
                            data = ((res >> (64-j)) & 1) as u16;
                        }

                        bus.store_halfword(self.dest_addr, data);

                        self.src_addr += 2;
                        self.dest_addr += 2;
                    }  
                }
            }
            else{
                println!("fatal error: eeprom DMA 3 has invalid config. chunksize: {}, src_inc: {}, dest_inc: {}", self.chunk_size as u32, self.src_increment as i32, self.dest_increment as i32);
            }
        }
        else if self.timing_mode != TimingMode::FIFO{
            for _ in 0..self.num_transfers {
                //println!("dest: {:#x}, src: {:#x}, data: {:#010x}", self.dest_addr, self.src_addr, bus.read_word(self.src_addr));
                match self.chunk_size {
                    ChunkSize::Halfword => {
                        let data = bus.read_halfword(self.src_addr);
                        bus.store_halfword(self.dest_addr, data);
                    }
                    ChunkSize::Word => {
                        let data = bus.read_word(self.src_addr);
                        bus.store_word(self.dest_addr, data);
                    }
                    _ => {
                        println!("DMA chunk size must be Word or Halfword");
                    }
                };
                self.src_addr += self.src_increment * self.chunk_size as usize;
                self.dest_addr += self.dest_increment * self.chunk_size as usize;
            }
        }
        else{
            let channel_num = (self.dest_addr- 0x040000a0) >> 2;
            for _ in 0..self.num_transfers{
                /*if self.dest_addr == 0x040000a0{
                    println!("src addr:     {:#x}", self.src_addr);
                }
                else{
                    println!("    src addr: {:#x}", self.src_addr);
                }*/
                match self.chunk_size {
                    ChunkSize::Word => {
                        let word = bus.read_word(self.src_addr);
                        bus.apu.direct_sound_fifo[channel_num].push_back((word & 0b11111111) as i8);
                        bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 8) & 0b11111111) as i8);
                        bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 16) & 0b11111111) as i8);
                        bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 24) & 0b11111111) as i8);
                    }
                    ChunkSize::Halfword => {
                        let halfword = bus.read_halfword(self.src_addr);
                        bus.apu.direct_sound_fifo[channel_num].push_back((halfword & 0b11111111) as i8);
                        bus.apu.direct_sound_fifo[channel_num].push_back(((halfword >> 8) & 0b11111111) as i8);
                    }
                    _ => {
                        println!("DMA chunk size must be Word or Halfword");
                    }
                };
                
                self.src_addr += self.src_increment * self.chunk_size as usize;
            }
        }

        // if not repeating, set inactive and clear the associated bit in memory
        if !self.is_repeating {
            self.is_enabled = false;
            let mut dma_cnt_upper =
                bus.read_byte_raw(0xbb + 12 * self.channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io});
            dma_cnt_upper &= !(1 << 7);
            bus.store_byte_raw(0xbb + 12 * self.channel_no, if IS_ARM9 {MemoryRegion::Arm9Io} else {MemoryRegion::Arm7Io}, dma_cnt_upper);
        }
        if self.raise_interrupt {
            bus.cpu_interrupt::<IS_ARM9>(1 << (8 + self.channel_no));
        }

        (self.num_transfers as u32 - 1) * 2 + 4
    }
}