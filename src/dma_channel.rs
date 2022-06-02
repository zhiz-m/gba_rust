#![allow(non_camel_case_types)]

use crate::bus::{Bus, ChunkSize};

#[derive(Clone, Copy, PartialEq)]
pub enum TimingMode{
    Immediate,
    VBlank,
    HBlank,
    FIFO,
}

#[derive(Clone)]
pub struct DMA_Channel{
    channel_no: usize,
    src_addr: usize,
    dest_addr: usize,
    src_increment: usize, // -1, 0, 1. 
    dest_increment: usize, // -1, 0, 1. 
    num_transfers: u16,
    chunk_size: ChunkSize,
    timing_mode: TimingMode,
    raise_interrupt: bool,
    is_repeating: bool,
    repeat_reset_dest: bool,
    pub is_enabled: bool,
}

impl DMA_Channel {
    pub fn new_disabled(channel_no: usize) -> DMA_Channel{
        DMA_Channel{
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
        let src_addr = bus.read_word(0x040000b0 + 12 * channel_no) as usize;
        let dest_addr = bus.read_word(0x040000b4 + 12 * channel_no) as usize;
        let dma_cnt = bus.read_word(0x040000b8 + 12 * channel_no);
        let mut is_enabled = true;
        let timing_mode = match (dma_cnt >> 0x1c) & 0b11 {
            0b00 => TimingMode::Immediate,
            0b01 => TimingMode::VBlank,
            0b10 => TimingMode::HBlank,
            0b11 => {
                // turn dma channel off
                is_enabled = false;
                let mut dma_cnt_upper = bus.read_byte_raw(0x040000bb + 12 * channel_no);
                dma_cnt_upper &= !(1 << 7);
                bus.store_byte_raw(0x040000bb + 12 * channel_no, dma_cnt_upper);
                TimingMode::FIFO
            },
            _ => panic!(),
        };
        if timing_mode != TimingMode::FIFO{
            //println!("dma channel {}, src_addr: {:#x}, dest addr: {:#x}, num_transfers: {:#x}", channel_no, src_addr, dest_addr, dma_cnt as u16);
        }
        //assert!(!is_enabled || dma_cnt as u16 > 0);
        DMA_Channel{
            channel_no,
            src_addr,
            dest_addr,
            src_increment: 0,
            dest_increment: 0,
            num_transfers: dma_cnt as u16,
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
                    match self.timing_mode{
                        TimingMode::Immediate => true,
                        TimingMode::HBlank => bus.hblank_dma,
                        TimingMode::VBlank => bus.vblank_dma,
                        TimingMode::FIFO => panic!("fifo is not supported and should not be enabled")
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
        let dma_cnt = bus.read_word_raw(0x040000b8 + 12 * self.channel_no);

        if self.is_repeating {
            self.num_transfers = dma_cnt as u16;
            if self.repeat_reset_dest {
                self.dest_addr = bus.read_word_raw(0x040000b4 + 12 * self.channel_no) as usize;
            }
        }

        //self.num_transfers = dma_cnt as u16;
        self.repeat_reset_dest = false;

        self.dest_increment = match (dma_cnt >> 0x15) & 0b11 {
            0b00 => 1,
            0b01 => !0, // -1
            0b10 => 0,
            0b11 => {
                self.repeat_reset_dest = true;
                1
            },
            _ => panic!(),
        };

        self.src_increment = match (dma_cnt >> 0x17) & 0b11 {
            0b00 => 1,
            0b01 => !0, // -1
            0b10 => 0,
            0b11 => panic!("illegal DMA channel src_increment of 0b11"),
            _ => panic!(),
        };

        self.chunk_size = match (dma_cnt >> 0x1a) & 1 > 0 {
            true => ChunkSize::Word,
            false => ChunkSize::Halfword,
        };

        self.raise_interrupt = (dma_cnt >> 0x1e) & 1 > 0;

        self.is_repeating = self.timing_mode != TimingMode::Immediate && (dma_cnt >> 0x19) & 1 > 0;

        for _ in 0..self.num_transfers{
            match self.chunk_size{
                ChunkSize::Halfword => bus.store_halfword(self.dest_addr, bus.read_halfword(self.src_addr)),
                ChunkSize::Word => bus.store_word(self.dest_addr, bus.read_word(self.src_addr)),
                _ => panic!("DMA chunk size must be Word or Halfword")
            };
            self.src_addr += self.src_increment * self.chunk_size as usize;
            self.dest_addr += self.dest_increment * self.chunk_size as usize;
        }

        // if not repeating, set inactive and clear the associated bit in memory
        if !self.is_repeating {
            self.is_enabled = false;
            let mut dma_cnt_upper = bus.read_byte_raw(0x040000bb + 12 * self.channel_no);
            dma_cnt_upper &= !(1 << 7);
            bus.store_byte_raw(0x040000bb + 12 * self.channel_no, dma_cnt_upper);
        }
        if self.raise_interrupt {
            bus.cpu_interrupt(1 << (8 + self.channel_no));
        }

        (self.num_transfers as u32 - 1) * 2 + 4
    }


}