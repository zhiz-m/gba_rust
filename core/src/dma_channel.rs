#![allow(non_camel_case_types)]

use log::warn;

use crate::bus::{Bus, ChunkSize, MemoryRegion};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimingMode {
    Immediate,
    VBlank,
    HBlank,
    Fifo,
}

#[derive(Clone)]
pub struct DMA_Channel {
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

impl DMA_Channel {
    pub fn new_disabled(channel_no: usize) -> DMA_Channel {
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

    pub fn new_enabled(channel_no: usize, bus: &mut Bus) -> DMA_Channel {
        let src_addr = bus.read_word_raw(0xb0 + 12 * channel_no, MemoryRegion::IO) as usize;
        let dest_addr = bus.read_word_raw(0xb4 + 12 * channel_no, MemoryRegion::IO) as usize;
        let dma_cnt = bus.read_word_raw(0xb8 + 12 * channel_no, MemoryRegion::IO);
        let mut num_transfers = dma_cnt as u16;
        let timing_mode = match (dma_cnt >> 0x1c) & 0b11 {
            0b00 => TimingMode::Immediate,
            0b01 => TimingMode::VBlank,
            0b10 => TimingMode::HBlank,
            0b11 => {
                // turn dma channel off
                //is_enabled = false;
                //let mut dma_cnt_upper = bus.read_byte_raw(0x040000bb + 12 * channel_no);
                //dma_cnt_upper &= !(1 << 7);
                //bus.store_byte_raw(0x040000bb + 12 * channel_no, dma_cnt_upper);
                assert!(dest_addr == 0x040000a0 || dest_addr == 0x040000a4);
                //info!("dma fifo addr: {:#x}", src_addr)
                num_transfers = 4;
                TimingMode::Fifo
            }
            _ => unreachable!(),
        };
        if timing_mode == TimingMode::Fifo {
            //info!("dma channel {}, src_addr: {:#x}, dest addr: {:#x}, num_transfers: {:#x}", channel_no, src_addr, dest_addr, dma_cnt as u16);
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
            is_enabled: true,
        }
    }

    #[inline(always)]
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
                    TimingMode::Fifo => {
                        match self.channel_no {
                            0 => {
                                warn!("FIFO channel is invalid for DMA channel_no of 0");
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
                                    let vcount = bus.read_byte_raw(0x5, MemoryRegion::IO);
                                    (2..162).contains(&vcount)
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
    #[inline(always)]
    pub fn execute_dma(&mut self, bus: &mut Bus) -> u32 {
        //self.src_addr = bus.read_word(0x040000b0 + 12 * self.channel_no) as usize;
        //self.dest_addr = bus.read_word(0x040000b4 + 12 * self.channel_no) as usize;
        //if !self.check_is_active(bus){
        //    return 0;
        //}
        let dma_cnt = bus.read_word_raw(0xb8 + 12 * self.channel_no, MemoryRegion::IO);

        if self.is_repeating {
            // if this is a repeat run, need to re-load the number of transfers
            self.num_transfers = match self.timing_mode {
                TimingMode::Fifo => 4,
                _ => dma_cnt as u16,
            };
            if self.repeat_reset_dest {
                self.dest_addr =
                    bus.read_word_raw(0xb4 + 12 * self.channel_no, MemoryRegion::IO) as usize;
            }
        }

        //self.num_transfers = dma_cnt as u16;
        self.repeat_reset_dest = false;

        self.dest_increment = match self.timing_mode {
            TimingMode::Fifo => 0,
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
                warn!("illegal DMA channel src_increment of 0b11");
                0
            }
            _ => unreachable!(),
        };

        if self.timing_mode != TimingMode::Fifo {
            //info!("non-fifo dma dest_addr: {:#x}", self.dest_addr);
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

        self.raise_interrupt = (dma_cnt >> 0x1e) & 1 > 0;

        self.is_repeating = self.timing_mode == TimingMode::Fifo
            || (self.timing_mode != TimingMode::Immediate && (dma_cnt >> 0x19) & 1 > 0);

        if self.channel_no != 1 && self.channel_no != 2 {
            //info!("dest: {:#x}, channel_no: {}", self.dest_addr, self.channel_no);
        }
        //if self.timing_mode != TimingMode::FIFO{
        for _ in 0..self.num_transfers {
            //info!("dest: {:#x}, src: {:#x}, data: {:#010x}", self.dest_addr, self.src_addr, bus.read_word(self.src_addr));
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
                    warn!("DMA chunk size must be Word or Halfword");
                }
            };
            self.src_addr += self.src_increment * self.chunk_size as usize;
            self.dest_addr += self.dest_increment * self.chunk_size as usize;
        }
        /*}
        else{
            let channel_num = (self.dest_addr- 0x040000a0) >> 2;
            for _ in 0..self.num_transfers{
                /*if self.dest_addr == 0x040000a0{
                    info!("src addr:     {:#x}", self.src_addr);
                }
                else{
                    info!("    src addr: {:#x}", self.src_addr);
                }*/
                let word = bus.read_word(self.src_addr);
                bus.apu.direct_sound_fifo[channel_num].push_back((word & 0b11111111) as i8);
                bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 8) & 0b11111111) as i8);
                bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 16) & 0b11111111) as i8);
                bus.apu.direct_sound_fifo[channel_num].push_back(((word >> 24) & 0b11111111) as i8);
                self.src_addr += self.src_increment * self.chunk_size as usize;
            }
        }*/

        // if not repeating, set inactive and clear the associated bit in memory
        if !self.is_repeating {
            self.is_enabled = false;
            let mut dma_cnt_upper =
                bus.read_byte_raw(0xbb + 12 * self.channel_no, MemoryRegion::IO);
            dma_cnt_upper &= !(1 << 7);
            bus.store_byte_raw(0xbb + 12 * self.channel_no, MemoryRegion::IO, dma_cnt_upper);
        }
        if self.raise_interrupt {
            bus.cpu_interrupt(1 << (8 + self.channel_no));
        }

        (self.num_transfers as u32 - 1) * 2 + 4
    }
}
