use std::ops::{Index, IndexMut};

use log::{info, warn};

use crate::{
    algorithm, apu::Apu, config, cp15::Cp15, cpu::Cpu, dma_channel::DMA_Channel, timer::Timer,
};

//const MEM_MAX: usize = 268435456;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChunkSize {
    Word = 4,
    Halfword = 2,
    Byte = 1,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MemoryRegion {
    Arm7Bios = 0,
    MainMemory = 1,
    SharedWram = 2,
    Arm7Io = 3,
    Palette = 4,
    VramBgA = 5,
    VramBgB = 6,
    VramObjA = 7,
    VramObjB = 8,
    VramLcdc = 9,
    Oam = 10,
    Cartridge = 11,
    CartridgeSram = 12,
    ITcm = 13,
    DTcm = 14,
    Arm9Bios = 15,
    Arm9Io = 16, // note: when there is shared IO, arm7 is chosen
    Arm7Wram = 17,
    Arm7WirelessWaitstates = 18,
    Illegal = 19,
    CartridgeUpper = 20,
    // todo: arm7 wifi registers
}

const MEM_REGION_COUNT: usize = 19;

const MEM_REGION_SIZES: [usize; MEM_REGION_COUNT] = [
    0x4000,    // Arm7Bios
    0x400000,  // MainMemory
    0x8000,    // SharedWram
    0x800,     // Arm7IO
    0x800,     // Palette
    0x80000,   // VramBgA
    0x20000,   // VramBgB
    0x40000,   // VramObjA
    0x20000,   // VramObjB
    0xA4000,   // VramLcdc
    0x800,     // OAM
    0x2000000, // Cartridge (size is not fixed)
    0x20000,   // cartridge sram (maybe remove later, might not be used)
    0x8000,    // ITcm
    0x4000,    // DTcm
    0x8000,    // Arm9Bios
    0x800,     // Arm9IO
    0x100,     // Arm7WirelessWaitstates: TODO
    0x10000,   // Arm7Wram
];

#[derive(Clone, Copy, PartialEq)]
pub enum CartridgeType {
    Eeprom512,
    Eeprom8192,
    Sram,
    Flash64,
    Flash128,
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
        Some(res) => match res {
            0 => CartridgeType::Sram,
            1 | 2 => CartridgeType::Flash64,
            3 => CartridgeType::Flash128,
            4 => CartridgeType::Eeprom8192,
            _ => unreachable!("logical error, invalid result from u8_search"),
        },
    }
}

/*
vec![0; 0x4000],
            vec![0; 0x40000],
            vec![0; 0x8000],
            vec![0; 0x400],
            vec![0; 0x400],
            vec![0; 0x18000],
            vec![0; 0x400],
            vec![0; 0x2000000],
            vec![0; 0x20000],
*/

// const MEM_REGION_OFFSET: [usize; 10] = [0x0, 0x4000, 0x44000, 0x4c000, 0x4c400, 0x4c800, 0x64800, 0x64c00, 0x2064c00, 0x2084c00];
// const MEM_REGION_TOTAL: usize = 0x2084c00;

struct FlatMemory {
    mem: Vec<u8>,
    mem_region_offset: [usize; MEM_REGION_COUNT + 1],
}

impl Default for FlatMemory {
    fn default() -> Self {
        let mut mem_region_offset = [0; MEM_REGION_COUNT + 1];
        for (i, num) in MEM_REGION_SIZES.iter().enumerate() {
            mem_region_offset[i + 1] += *num;
        }
        Self {
            mem: vec![0; mem_region_offset[MEM_REGION_COUNT]],
            mem_region_offset,
        }
    }
}

impl Index<usize> for FlatMemory {
    type Output = [u8];

    fn index(&self, index: usize) -> &Self::Output {
        &self.mem[self.mem_region_offset[index]..self.mem_region_offset[index + 1]]
    }
}

impl IndexMut<usize> for FlatMemory {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.mem[self.mem_region_offset[index]..self.mem_region_offset[index + 1]]
    }
}

impl Index<(usize, usize)> for FlatMemory {
    type Output = u8;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.mem[self.mem_region_offset[index.0] + index.1]
    }
}

impl IndexMut<(usize, usize)> for FlatMemory {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        &mut self.mem[self.mem_region_offset[index.0] + index.1]
    }
}

impl FlatMemory {}

pub struct Bus {
    mapped_mem: FlatMemory,

    pub cartridge_type: CartridgeType,

    // 0-2: cartridge command flags
    // 3: cartridge page number (for 218kb only, 0 or 1)
    // 4: cartridge mode
    //     val=0 read mode
    //     val=1 device&manufacturer info mode
    //     val=2: erase mode
    //     val=3: write single byte mode
    //     val=4: select page number mode
    cartridge_type_state: [u8; 7],
    pub eeprom_is_read: bool,
    pub eeprom_read_offset: usize,
    pub eeprom_write_successful: bool,

    pub is_any_arm9_dma_active: bool,
    pub is_any_arm7_dma_active: bool,
    pub hblank_dma: bool,
    pub vblank_dma: bool,
    pub dma_channels_arm9: [DMA_Channel<true>; 4],
    pub dma_channels_arm7: [DMA_Channel<false>; 4],

    pub is_any_timer_active: bool,
    arm9_timers: [Timer<true>; 4],
    arm7_timers: [Timer<false>; 4],

    pub arm9: Cpu<true>,
    pub arm7: Cpu<false>,
    // pub arm7: Cpu<false>,
    pub cp15: Cp15,

    pub dtcm_load_mode: bool,
    pub itcm_load_mode: bool,
    pub dtcm_addr: usize,
    pub dtcm_size: usize,

    pub tcm_readable: bool,

    pub apu: Apu,
}

impl Bus {
    pub fn new(
        bios_bin: &[u8],
        rom_bin: &[u8],
        save_state: Option<&[u8]>,
        cartridge_type_str: Option<&str>,
        apu: Apu,
    ) -> Bus {
        //let mut mem = vec![0; MEM_MAX];

        // let mut mapped_mem = [
        //     vec![0; 0x4000],
        //     vec![0; 0x40000],
        //     vec![0; 0x8000],
        //     vec![0; 0x400],
        //     vec![0; 0x400],
        //     vec![0; 0x18000],
        //     vec![0; 0x400],
        //     vec![0; 0x2000000],
        //     vec![0; 0x20000],
        // ];

        let mut mapped_mem = FlatMemory::default();

        // load BIOS
        //let bios_path = env::var("GBA_RUST_BIOS").unwrap();
        /*let mut reader = BufReader::new(File::open(bios_path).unwrap());
        reader.read(&mut mapped_mem[MemoryRegion::BIOS as usize][..]).unwrap();

        // load ROM
        let mut reader = BufReader::new(File::open(rom_path).unwrap());
        reader.read(&mut mapped_mem[MemoryRegion::Cartridge as usize][..]).unwrap();*/
        mapped_mem[MemoryRegion::Arm7Bios as usize][..].copy_from_slice(bios_bin);
        // TODO: load arm9 bios
        mapped_mem[MemoryRegion::Cartridge as usize][..rom_bin.len()].copy_from_slice(rom_bin);

        let cartridge_type = match cartridge_type_str {
            None => derive_cartridge_type(&mapped_mem[MemoryRegion::Cartridge as usize][..]),
            Some(cartridge_type_str) => {
                let cartridge_type_str = cartridge_type_str.trim().to_ascii_uppercase();
                let trimmed_str = cartridge_type_str.split(' ').next().unwrap();
                match trimmed_str {
                    "SRAM" => CartridgeType::Sram,
                    "FLASH" => CartridgeType::Flash64,
                    "FLASH512" => CartridgeType::Flash64,
                    "FLASH1M" => CartridgeType::Flash128,
                    "EEPROM512" => CartridgeType::Eeprom512,
                    "EEPROM8192" => CartridgeType::Eeprom8192,
                    _ => unreachable!(),
                }
            }
        };

        // load save state
        if let Some(buf) = save_state {
            mapped_mem[MemoryRegion::CartridgeSram as usize][..].copy_from_slice(buf);
        }

        info!("backup type: {}", cartridge_type as u32);

        Bus {
            mapped_mem,

            cartridge_type,
            cartridge_type_state: [0; 7],
            eeprom_is_read: false,
            eeprom_read_offset: 0,
            eeprom_write_successful: false,

            is_any_arm9_dma_active: false,
            is_any_arm7_dma_active: false,
            hblank_dma: false,
            vblank_dma: false,
            dma_channels_arm9: [
                DMA_Channel::new_disabled(0),
                DMA_Channel::new_disabled(1),
                DMA_Channel::new_disabled(2),
                DMA_Channel::new_disabled(3),
            ],
            dma_channels_arm7: [
                DMA_Channel::new_disabled(0),
                DMA_Channel::new_disabled(1),
                DMA_Channel::new_disabled(2),
                DMA_Channel::new_disabled(3),
            ],

            is_any_timer_active: false,
            arm9_timers: [Timer::new(0), Timer::new(1), Timer::new(2), Timer::new(3)],
            arm7_timers: [Timer::new(0), Timer::new(1), Timer::new(2), Timer::new(3)],

            arm9: Cpu::<true>::new(),
            arm7: Cpu::<false>::new(),
            cp15: Default::default(),

            dtcm_load_mode: false,
            itcm_load_mode: false,
            dtcm_addr: 0,
            dtcm_size: 0,

            tcm_readable: true,

            apu,
        }
    }

    // -------- public memory read/write interfaces, intended for user instructions.

    #[inline(always)]
    pub fn read_byte(&mut self, addr: usize) -> u8 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Byte, true);
        self.internal_read_byte(addr, region)
    }

    #[inline(always)]
    pub fn read_halfword(&mut self, addr: usize) -> u16 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Halfword, true);
        assert!(addr & 1 == 0);
        self.internal_read_byte(addr, region) as u16
            + ((self.internal_read_byte(addr + 1, region) as u16) << 8)
    }

    #[inline(always)]
    pub fn read_word(&mut self, addr: usize) -> u32 {
        let (addr, region) = self.addr_match(addr, ChunkSize::Word, true);
        assert!(addr & 0b11 == 0);
        self.internal_read_byte(addr, region) as u32
            + ((self.internal_read_byte(addr + 1, region) as u32) << 8)
            + ((self.internal_read_byte(addr + 2, region) as u32) << 16)
            + ((self.internal_read_byte(addr + 3, region) as u32) << 24)
    }

    #[inline(always)]
    pub fn store_byte(&mut self, addr: usize, val: u8) {
        let (addr, region) = self.addr_match(addr, ChunkSize::Byte, false);
        self.internal_write_byte(addr, region, val);
    }

    #[inline(always)]
    pub fn store_halfword(&mut self, addr: usize, val: u16) {
        let (addr, region) = self.addr_match(addr, ChunkSize::Halfword, false);
        assert!(addr & 1 == 0);
        self.internal_write_byte(addr, region, (val & 0b11111111) as u8);
        self.internal_write_byte(addr + 1, region, ((val >> 8) & 0b11111111) as u8);
    }

    #[inline(always)]
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

    #[inline(always)]
    pub fn read_byte_raw(&self, addr: usize, region: MemoryRegion) -> u8 {
        self.mapped_mem[(region as usize, addr)]
    }

    #[inline(always)]
    pub fn read_halfword_raw(&self, addr: usize, region: MemoryRegion) -> u16 {
        self.mapped_mem[(region as usize, addr)] as u16
            + ((self.mapped_mem[(region as usize, addr + 1)] as u16) << 8)
    }

    #[inline(always)]
    pub fn read_word_raw(&self, addr: usize, region: MemoryRegion) -> u32 {
        self.mapped_mem[(region as usize, addr)] as u32
            + ((self.mapped_mem[(region as usize, addr + 1)] as u32) << 8)
            + ((self.mapped_mem[(region as usize, addr + 2)] as u32) << 16)
            + ((self.mapped_mem[(region as usize, addr + 3)] as u32) << 24)
    }

    #[inline(always)]
    pub fn store_byte_raw(&mut self, addr: usize, region: MemoryRegion, val: u8) {
        self.mapped_mem[(region as usize, addr)] = val;
    }

    #[inline(always)]
    pub fn store_halfword_raw(&mut self, addr: usize, region: MemoryRegion, val: u16) {
        self.mapped_mem[(region as usize, addr)] = (val & 0b11111111) as u8;
        self.mapped_mem[(region as usize, addr + 1)] = ((val >> 8) & 0b11111111) as u8;
    }

    #[inline(always)]
    pub fn store_word_raw(&mut self, addr: usize, region: MemoryRegion, val: u32) {
        self.mapped_mem[(region as usize, addr)] = (val & 0b11111111) as u8;
        self.mapped_mem[(region as usize, addr + 1)] = ((val >> 8) & 0b11111111) as u8;
        self.mapped_mem[(region as usize, addr + 2)] = ((val >> 16) & 0b11111111) as u8;
        self.mapped_mem[(region as usize, addr + 3)] = ((val >> 24) & 0b11111111) as u8;
    }

    // -------- miscellaneous public methods to communicate with other components of GBA system
    #[inline(always)]
    pub fn cpu_interrupt<const IS_ARM9: bool>(&mut self, interrupt: u32) {
        let region = if IS_ARM9 {
            MemoryRegion::Arm9Io
        } else {
            MemoryRegion::Arm7Io
        };
        let reg_if = self.read_word_raw(0x214, region);

        // AND with reg_ie
        let cur_reg_if = interrupt & self.read_word_raw(0x210, region);
        // self.store_halfword(0x04000202, cur_reg_if & !(reg_if));
        self.mapped_mem[(region as usize, 0x214)] ^= (cur_reg_if & !(reg_if)) as u8;
        self.mapped_mem[(region as usize, 0x215)] ^= ((cur_reg_if & !(reg_if)) >> 8) as u8;
        self.mapped_mem[(region as usize, 0x216)] ^= ((cur_reg_if & !(reg_if)) >> 16) as u8;
        self.mapped_mem[(region as usize, 0x217)] ^= ((cur_reg_if & !(reg_if)) >> 24) as u8;

        // clear some interrupts that we dont want
        if IS_ARM9 {
            self.mapped_mem[(region as usize, 0x214)] &= !0b10000000;
        }
        // clear gba-slot bit and unused bits
        self.mapped_mem[(region as usize, 0x215)] &= !0b11100000;
        if !IS_ARM9 {
            // clear geometry command fifo
            self.mapped_mem[(region as usize, 0x216)] &= !0b00100000;
            self.mapped_mem[(region as usize, 0x217)] &= 0b00000001;
        } else {
            // clear 4 other geometry bits
            self.mapped_mem[(region as usize, 0x216)] &= !0b11100000;
            self.mapped_mem[(region as usize, 0x217)] = 0;
        }

        if IS_ARM9 {
            self.arm9.interrupt_requested = self.arm9.check_interrupt(self);
        } else {
            self.arm7.interrupt_requested = self.arm7.check_interrupt(self);
        }
    }

    #[inline(always)]
    pub fn timer_clock(&mut self) {
        if !self.is_any_timer_active {
            return;
        }
        unsafe {
            // arm9 timers
            for i in 0..4 {
                let ptr = &mut self.arm9_timers[i] as *mut Timer<true>;
                if (*ptr).is_enabled
                    && (*ptr).clock(self)
                    && i != 3
                    && self.arm9_timers[i + 1].is_cascading
                {
                    let ptr = &mut self.arm9_timers[i + 1] as *mut Timer<true>;
                    (*ptr).cascade();
                }
            }

            // arm7 timers
            for i in 0..4 {
                let ptr = &mut self.arm7_timers[i] as *mut Timer<false>;
                if (*ptr).is_enabled
                    && (*ptr).clock(self)
                    && i != 3
                    && self.arm7_timers[i + 1].is_cascading
                {
                    let ptr = &mut self.arm7_timers[i + 1] as *mut Timer<false>;
                    (*ptr).cascade();
                }
            }
        }
    }

    #[inline(always)]
    pub fn arm9_clock(&mut self) -> u32 {
        let ptr = &mut self.arm9 as *mut Cpu<true>;
        unsafe { (*ptr).clock(self) }
    }

    #[inline(always)]
    pub fn arm7_clock(&mut self) -> u32 {
        let ptr = &mut self.arm7 as *mut Cpu<false>;
        unsafe { (*ptr).clock(self) }
    }

    // note: for clarify, channels 1-4 will be representing using numbers 0-3
    #[inline(always)]
    pub fn apu_clock(&mut self) {
        let ptr = &mut self.apu as *mut Apu;
        unsafe {
            (*ptr).clock(self);
        }
    }

    #[inline(always)]
    pub fn export_sram(&self, buff: &mut [u8]) {
        buff.copy_from_slice(&self.mapped_mem[MemoryRegion::CartridgeSram as usize][..]);
    }

    // -------- helper functions
    #[inline(always)]
    pub fn set_is_any_dma_active<const IS_ARM9: bool>(&mut self) {
        if IS_ARM9 {
            self.is_any_arm9_dma_active = false;
            for i in 0..4 {
                if self.dma_channels_arm9[i].is_enabled {
                    self.is_any_arm9_dma_active = true;
                    return;
                }
            }
        } else {
            self.is_any_arm7_dma_active = false;
            for i in 0..4 {
                if self.dma_channels_arm7[i].is_enabled {
                    self.is_any_arm7_dma_active = true;
                    return;
                }
            }
        };
    }

    #[inline(always)]
    pub fn set_is_any_timer_active(&mut self) {
        self.is_any_timer_active = false;
        for i in 0..4 {
            if self.arm9_timers[i].is_enabled {
                self.is_any_timer_active = true;
                return;
            }
        }
        for i in 0..4 {
            if self.arm7_timers[i].is_enabled {
                self.is_any_timer_active = true;
                return;
            }
        }
    }

    #[inline(always)]
    fn internal_read_byte(&mut self, addr: usize, region: MemoryRegion) -> u8 {
        match region {
            MemoryRegion::Arm9Io => {
                if (0x100..=0x10e).contains(&addr) {
                    match addr {
                        0x100 => self.arm9_timers[0].timer_count as u8,
                        0x101 => (self.arm9_timers[0].timer_count >> 8) as u8,
                        0x104 => self.arm9_timers[1].timer_count as u8,
                        0x105 => (self.arm9_timers[1].timer_count >> 8) as u8,
                        0x108 => self.arm9_timers[2].timer_count as u8,
                        0x109 => (self.arm9_timers[2].timer_count >> 8) as u8,
                        0x10c => self.arm9_timers[3].timer_count as u8,
                        0x10d => (self.arm9_timers[3].timer_count >> 8) as u8,
                        _ => self.mapped_mem[(region as usize, addr)],
                    }
                } else {
                    self.mapped_mem[(region as usize, addr)]
                }
            }
            MemoryRegion::Arm7Io => {
                if (0x100..=0x10e).contains(&addr) {
                    match addr {
                        0x100 => self.arm7_timers[0].timer_count as u8,
                        0x101 => (self.arm7_timers[0].timer_count >> 8) as u8,
                        0x104 => self.arm7_timers[1].timer_count as u8,
                        0x105 => (self.arm7_timers[1].timer_count >> 8) as u8,
                        0x108 => self.arm7_timers[2].timer_count as u8,
                        0x109 => (self.arm7_timers[2].timer_count >> 8) as u8,
                        0x10c => self.arm7_timers[3].timer_count as u8,
                        0x10d => (self.arm7_timers[3].timer_count >> 8) as u8,
                        _ => self.mapped_mem[(region as usize, addr)],
                    }
                } else {
                    self.mapped_mem[(region as usize, addr)]
                }
            }
            MemoryRegion::Arm7Bios => {
                let offset = (addr & 0b11) << 3;
                //let range = 0b11111111 << (offset);
                if self.arm7.actual_pc >= 0x4000 {
                    warn!(
                        "attempt for arm7 CPU to read BIOS from outside, {} {:#x}",
                        offset, self.arm7.last_fetched_bios_instr
                    );
                    ((self.arm7.last_fetched_bios_instr >> offset) & 0b11111111) as u8
                } else {
                    //self.cpu.last_fetched_bios_instr &= !range;
                    //self.cpu.last_fetched_bios_instr = (self.mapped_mem[region as usize][addr] as u32) << offset;
                    self.mapped_mem[(region as usize, addr)]
                }
            }
            MemoryRegion::Arm9Bios => {
                let offset = (addr & 0b11) << 3;
                //let range = 0b11111111 << (offset);
                if self.arm9.actual_pc >= 0x800 {
                    warn!(
                        "attempt for arm9 CPU to read BIOS from outside, {} {:#x}",
                        offset, self.arm9.last_fetched_bios_instr
                    );
                    ((self.arm9.last_fetched_bios_instr >> offset) & 0b11111111) as u8
                } else {
                    //self.cpu.last_fetched_bios_instr &= !range;
                    //self.cpu.last_fetched_bios_instr = (self.mapped_mem[region as usize][addr] as u32) << offset;
                    self.mapped_mem[(region as usize, addr)]
                }
            }
            MemoryRegion::Illegal => {
                let range = (addr & 0b11) << 3;
                (self.arm9.pipeline_instr.get(1).unwrap() >> range) as u8
            }
            _ => self.mapped_mem[(region as usize, addr)],
        }
    }

    #[inline(always)]
    fn internal_write_byte(&mut self, addr: usize, region: MemoryRegion, val: u8) {
        match region {
            MemoryRegion::Arm9Io | MemoryRegion::Arm7Io => {
                if (0x65..=0x301).contains(&addr) {
                    match addr {
                        0x301 => {
                            if region == MemoryRegion::Arm7Io{
                                if val >> 6 > 0 {
                                    // todo: add handling for STOP state (pause sound, PPU and cpu, gba mode)
                                } else {
                                    // request that CPU is paused until next interrupt
                                    self.arm9.halt();
                                }
                            }
                        }

                        0x208 => {
                            if region == MemoryRegion::Arm9Io {
                                self.arm9.interrupt_requested = self.arm9.check_interrupt(self);
                            }
                            else{
                                self.arm7.interrupt_requested = self.arm7.check_interrupt(self);
                            }
                        }

                        // special handling for REG_IF, interrupt handling
                        0x202 | 0x203 => {
                            // current bit 0, incoming bit 0 -> result = 0
                            // current bit 1, incoming bit 1 -> result = 0
                            // current bit 1, incoming bit 0 -> result = 1
                            // current bit 0, incoming bit 1 -> result = 1
                            /* self.mapped_mem[(region as usize,addr)] ^= val;
                            self.mapped_mem[region as usize][0x202] &= !0b10000000;
                            self.mapped_mem[region as usize][0x203] &= !0b00100000;
                            self.cpu.interrupt_requested = self.cpu.check_interrupt(self);
                            */
                            //let old = self.mapped_mem[region as usize][addr];

                            // only allow turning off interrupts through CPU
                            self.mapped_mem[(region as usize, addr)] =
                                (self.mapped_mem[(region as usize, addr)] ^ val)
                                    & self.mapped_mem[(region as usize, addr)];
                            if region == MemoryRegion::Arm9Io {
                                self.arm9.interrupt_requested = self.arm9.check_interrupt(self);
                            }
                            else{
                                self.arm7.interrupt_requested = self.arm7.check_interrupt(self);
                            }
                            return;
                        }

                        // special handling for DMA
                        0xbb | 0xc7 | 0xd3 | 0xdf => {
                            let old_val = self.mapped_mem[(region as usize, addr)];
                            self.mapped_mem[(region as usize, addr)] = val;
                            let channel_no = (addr - 0xbb) / 12;
                            //info!("addr: {:#x}, val: {:#010b}, channel_no: {}", addr, val, channel_no);
                            if region == MemoryRegion::Arm9Io{
                                let dma_channel = if val >> 7 > 0 && old_val >> 7 & 1 == 0 {
                                    DMA_Channel::new_enabled(channel_no, self)
                                    //info!("enabled dma, bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}", addr, val, channel_no, res.dest_addr);
                                } else if val >> 7 == 0 {
                                    //info!("disabled dma, addr: {:#x}, val: {:#010b}, channel_no: {}", addr, val, channel_no);
                                    DMA_Channel::new_disabled(channel_no)
                                } else {
                                    //let res = DMA_Channel::new_enabled(channel_no, self);
                                    //if res.timing_mode != self.dma_channels[channel_no].timing_mode{
                                    //info!("debug dma new: bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}, src_addr: {:#x}, timing_mode: {}", addr, val, channel_no, res.dest_addr, res.src_addr, res.timing_mode as u32);
                                    //info!("debug dma old: bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}, src_addr: {:#x}, timing_mode: {}", addr, old_val, channel_no, self.dma_channels[channel_no].dest_addr, self.dma_channels[channel_no].src_addr, self.dma_channels[channel_no].timing_mode as u32);
                                    //}
                                    return;
                                };
                                self.dma_channels_arm9[channel_no] = dma_channel;
                                self.set_is_any_dma_active::<true>();
                            }
                            else{
                                let dma_channel = if val >> 7 > 0 && old_val >> 7 & 1 == 0 {
                                    DMA_Channel::new_enabled(channel_no, self)
                                    //info!("enabled dma, bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}", addr, val, channel_no, res.dest_addr);
                                } else if val >> 7 == 0 {
                                    //info!("disabled dma, addr: {:#x}, val: {:#010b}, channel_no: {}", addr, val, channel_no);
                                    DMA_Channel::new_disabled(channel_no)
                                } else {
                                    //let res = DMA_Channel::new_enabled(channel_no, self);
                                    //if res.timing_mode != self.dma_channels[channel_no].timing_mode{
                                    //info!("debug dma new: bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}, src_addr: {:#x}, timing_mode: {}", addr, val, channel_no, res.dest_addr, res.src_addr, res.timing_mode as u32);
                                    //info!("debug dma old: bus addr: {:#x}, val: {:#010b}, channel_no: {}, dest_addr: {:#x}, src_addr: {:#x}, timing_mode: {}", addr, old_val, channel_no, self.dma_channels[channel_no].dest_addr, self.dma_channels[channel_no].src_addr, self.dma_channels[channel_no].timing_mode as u32);
                                    //}
                                    return;
                                };
                                self.dma_channels_arm7[channel_no] = dma_channel;
                                self.set_is_any_dma_active::<false>();
                            }
                            
                            //info!("set dma flags");
                            return;
                        }

                        // special handling for writing to timer count
                        0x100 | 0x101 | 0x104 | 0x105 | 0x108 | 0x109 | 0x10c | 0x10d => {
                            let timer_no = (addr - 0x100) >> 2;
                            if region == MemoryRegion::Arm9Io{
                                unsafe {
                                    let ptr = &mut self.arm9_timers[timer_no] as *mut Timer<true>;
                                    if addr & 1 == 0 {
                                        (*ptr).reload_val &= !0b11111111;
                                        (*ptr).reload_val |= val as u16;
                                    } else {
                                        (*ptr).reload_val &= 0b11111111;
                                        (*ptr).reload_val |= (val as u16) << 8;
                                    }
                                }
                            }
                            else{
                                unsafe {
                                    let ptr = &mut self.arm7_timers[timer_no] as *mut Timer<false>;
                                    if addr & 1 == 0 {
                                        (*ptr).reload_val &= !0b11111111;
                                        (*ptr).reload_val |= val as u16;
                                    } else {
                                        (*ptr).reload_val &= 0b11111111;
                                        (*ptr).reload_val |= (val as u16) << 8;
                                    }
                                }
                            }
                        }

                        // special handling for timer control
                        0x102 | 0x106 | 0x10a | 0x10e => {
                            let timer_no = (addr - 0x102) >> 2;
                            unsafe {
                                if region == MemoryRegion::Arm9Io{
                                    let ptr = &mut self.arm9_timers[timer_no] as *mut Timer<true>;
                                    (*ptr).set_period(val & 0b11);
                                    (*ptr).is_cascading = (val >> 2) & 1 > 0;
                                    (*ptr).raise_interrupt = (val >> 6) & 1 > 0;
                                    (*ptr).set_is_enabled((val >> 7) & 1 > 0);
                                }
                                else{
                                    let ptr = &mut self.arm7_timers[timer_no] as *mut Timer<false>;
                                    (*ptr).set_period(val & 0b11);
                                    (*ptr).is_cascading = (val >> 2) & 1 > 0;
                                    (*ptr).raise_interrupt = (val >> 6) & 1 > 0;
                                    (*ptr).set_is_enabled((val >> 7) & 1 > 0);
                                }
                                self.set_is_any_timer_active();
                            }
                        }

                        // NOTE: below is old gba sound. turned off for now. 

                        /*
                        // special handling for square sound channels; reset
                        0x65 | 0x6d => {
                            self.mapped_mem[(region as usize, addr)] = val;
                            let square_chan_num = match addr {
                                0x65 => 0,
                                0x6d => 1,
                                _ => unreachable!(),
                            };
                            if (val >> 7) & 1 > 0 {
                                let ptr = &mut self.apu as *mut Apu;
                                unsafe {
                                    (*ptr).reset_square_channel(square_chan_num, self);
                                };
                            }
                            return;
                        }

                        // special handling for wave sound channel (official name: DMG channel 3)
                        0x75 => {
                            self.mapped_mem[(region as usize, addr)] = val;
                            if (val >> 7) & 1 > 0 {
                                let ptr = &mut self.apu as *mut Apu;
                                unsafe {
                                    (*ptr).reset_wave_channel(self);
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
                        // special handling for direct sound channels; reset
                        0x83 => {
                            for i in 0..2 {
                                if (val >> (3 + 4 * i)) & 1 == 0 {
                                    continue;
                                }
                                let enable_right_left =
                                    [(val >> (4 * i)) & 1 > 0, (val >> (1 + 4 * i)) & 1 > 0];
                                if !enable_right_left[0] && !enable_right_left[1] {
                                    self.apu.direct_sound_timer[i] = None;
                                } else {
                                    self.apu.direct_sound_timer[i] =
                                        Some((val as usize >> (2 + i * 4)) & 1);
                                }
                                self.apu.direct_sound_fifo[i].clear();
                                self.apu.direct_sound_fifo_cur[i] = 0;
                            }
                        }

                        // special handling for direct sound FIFO insertions
                        0xa0..=0xa7 => {
                            let channel_num = (addr - 0xa0) >> 2;
                            if self.apu.direct_sound_fifo[channel_num].len() < 32 {
                                self.apu.direct_sound_fifo[channel_num].push_back(val as i8);
                            } else {
                                //self.apu.direct_sound_fifo[channel_num].pop_back();
                                //self.apu.direct_sound_fifo[channel_num].push_back(val as i8);
                                warn!(
                                    "sound fifo: {}, attempt to add sample at 32 capacity",
                                    channel_num
                                );
                            }
                            // do not write to mem directly
                            return;
                        }

                        // special handling for inserting into wave sound channel bank
                        0x90..=0x9f => {
                            let ind = addr - 0x90;
                            let bank = (self.mapped_mem[(region as usize, 0x70)] >> 5)
                                & !(self.mapped_mem[(region as usize, 0x70)] >> 6)
                                & 1;
                            self.apu.wave_bank[bank as usize][ind] = val;

                            // do not write to mem directly
                            return;
                        }

                        // special handling for wave channel disable/enable
                        0x70 => {
                            if (val ^ self.mapped_mem[(region as usize, addr)]) >> 7 > 0 {
                                self.apu.wave_sweep_cnt = 0;
                            }
                        }
                        

                        0x84 => {
                            if (val >> 7) & 1 == 0 {
                                for i in 0x60..=0x81 {
                                    self.mapped_mem[(region as usize, i)] = 0;
                                }
                            }
                        }
                        _ => {}
                        */
                    }
                }
                self.mapped_mem[(region as usize, addr)] = val;
            }
            MemoryRegion::Arm7Bios | MemoryRegion::Arm9Bios => {
                // do nothing, writing to BIOS is illegal
            }
            MemoryRegion::CartridgeSram => {
                //info!("write to SRAM, addr: {:#x}, val: {:#x}", addr, val);
                match self.cartridge_type {
                    CartridgeType::Flash64 | CartridgeType::Flash128 => {
                        self.internal_write_byte_flash(addr, val);
                    }
                    CartridgeType::Sram => {
                        self.mapped_mem[(region as usize, addr)] = val;
                    }
                    _ => {
                        warn!(
                            "writing to SRAM is forbidden for cartridge type {}",
                            self.cartridge_type as u32
                        );
                    }
                }
            }
            MemoryRegion::Illegal => {
                //warn!("illegal memory write");
            }
            _ => {
                self.mapped_mem[(region as usize, addr)] = val;
            }
        };
    }

    fn internal_read_byte_flash(&self, addr: usize) -> u8 {
        match self.cartridge_type_state[4] {
            //0 => {

            //},
            1 => {
                let (device, man) = match self.cartridge_type {
                    CartridgeType::Flash64 => {
                        (0x1c, 0xc2) // Macronix 64kb
                    }
                    CartridgeType::Flash128 => {
                        (0x09, 0xc2) // Macronix 128kb
                    }
                    _ => unreachable!("cartridge type is not flash"),
                };
                match addr {
                    0x0 => man,
                    0x1 => device,
                    _ => {
                        warn!("invalid addr for read in device/manufacturer mode");
                        0
                    }
                }
            }
            _ => match self.cartridge_type {
                CartridgeType::Flash64 => {
                    self.mapped_mem[(
                        MemoryRegion::CartridgeSram as usize,
                        config::FLASH64_MEM_START + (addr & 0xffff),
                    )]
                }
                CartridgeType::Flash128 => {
                    self.mapped_mem[(
                        MemoryRegion::CartridgeSram as usize,
                        config::FLASH128_MEM_START
                            + (addr & 0xffff)
                            + ((self.cartridge_type_state[3] as usize) << 16),
                    )]
                }
                _ => unreachable!("cartridge type is not flash"),
            },
        }
    }

    fn internal_write_byte_flash(&mut self, addr: usize, val: u8) {
        match self.cartridge_type_state[4] {
            // write single byte
            3 => {
                match self.cartridge_type {
                    CartridgeType::Flash64 => {
                        self.mapped_mem[(
                            MemoryRegion::CartridgeSram as usize,
                            config::FLASH64_MEM_START + (addr & 0xffff),
                        )] = val;
                    }
                    CartridgeType::Flash128 => {
                        self.mapped_mem[(
                            MemoryRegion::CartridgeSram as usize,
                            config::FLASH128_MEM_START
                                + (addr & 0xffff)
                                + ((self.cartridge_type_state[3] as usize) << 16),
                        )] = val;
                    }
                    _ => unreachable!("cartridge type is not flash"),
                }
                self.cartridge_type_state[4] = 0;
            }
            _ => {
                if addr == 0x5555 {
                    if val == 0xaa {
                        self.cartridge_type_state[0] = val;
                        self.cartridge_type_state[2] = 0;
                    } else if self.cartridge_type_state[1] != 0 {
                        self.cartridge_type_state[2] = val;
                        self.execute_flash_storage_command();
                    }
                } else if addr == 0x2aaa {
                    self.cartridge_type_state[1] = val;
                } else {
                    match self.cartridge_type_state[4] {
                        2 => {
                            if addr & 0xfff == 0
                                && self.cartridge_type_state[0] != 0
                                && self.cartridge_type_state[1] != 0
                                && val == 0x30
                            {
                                // special: erase entire sector
                                //info!("sector erase: {:#x}", addr);
                                let addr = match self.cartridge_type {
                                    CartridgeType::Flash64 => {
                                        config::FLASH64_MEM_START + (addr & 0xffff)
                                    }
                                    CartridgeType::Flash128 => {
                                        config::FLASH128_MEM_START
                                            + (addr & 0xffff)
                                            + ((self.cartridge_type_state[3] as usize) << 16)
                                    }
                                    _ => unreachable!("cartridge type is not flash"),
                                };
                                for i in addr..addr + 0x1000 {
                                    self.mapped_mem[(MemoryRegion::CartridgeSram as usize, i)] =
                                        0xff;
                                }
                                self.cartridge_type_state[0] = 0;
                                self.cartridge_type_state[1] = 0;
                                self.cartridge_type_state[2] = 0;
                                self.cartridge_type_state[4] = 0;
                            }
                        }
                        // bank switching
                        4 => {
                            assert!(addr == 0x0);
                            if addr == 0x0 {
                                assert!(val <= 1);
                                self.cartridge_type_state[3] = val;
                                self.cartridge_type_state[4] = 0;
                            }
                        }
                        _ => warn!(
                            "invalid cartridge type state for write: {}",
                            self.cartridge_type_state[4]
                        ),
                    }
                }
            }
        }
    }

    fn execute_flash_storage_command(&mut self) {
        match self.cartridge_type_state[2] {
            0x90 => {
                self.cartridge_type_state[4] = 1;
            }
            0xf0 => {
                if self.cartridge_type_state[4] == 1 {
                    self.cartridge_type_state[4] = 0;
                }
            }
            0x80 => {
                self.cartridge_type_state[4] = 2;
            }
            0x10 => {
                if self.cartridge_type_state[4] == 1 {
                    let (start, end) = match self.cartridge_type{
                        CartridgeType::Flash64 => (config::FLASH64_MEM_START, config::FLASH64_MEM_END),
                        CartridgeType::Flash128 => (config::FLASH128_MEM_START, config::FLASH128_MEM_END),
                        _ => unreachable!("logical error: execute_flash_storage_command is caled, but cartridge type is not FLASH64 or FLASH128"),
                    };
                    for i in start..end {
                        self.mapped_mem[(MemoryRegion::CartridgeSram as usize, i)] = 0xff;
                    }
                }
                self.cartridge_type_state[4] = 0;
            }
            0xa0 => {
                self.cartridge_type_state[4] = 3;
            }
            0xb0 => {
                self.cartridge_type_state[4] = 4;
            }
            _ => {}
        }
        self.cartridge_type_state[0] = 0;
        self.cartridge_type_state[1] = 0;
        self.cartridge_type_state[2] = 0;
    }

    #[inline(always)]
    fn addr_match<const IS_ARM9: bool>(
        &self,
        addr: usize,
        chunk_size: ChunkSize,
        is_read: bool,
    ) -> (usize, MemoryRegion) {
        //if addr >= 0x4000000 && addr < 0x4700000 {
        //    return (addr % 0x0010000) + 0x4000000;
        //}

        if IS_ARM9
            && self.tcm_readable
            && addr >= self.dtcm_addr
            && addr < self.dtcm_addr + self.dtcm_size
        {
            return (addr - self.dtcm_addr, MemoryRegion::DTcm);
        }

        match addr >> 24 {
            0 | 1 => {
                if IS_ARM9 {
                    (addr & 0x7fff, MemoryRegion::ITcm)
                } else if addr >= 0x4000 {
                    #[cfg(feature = "debug_instr")]
                    warn!("illegal memory address: {:#x}", addr);
                    (addr, MemoryRegion::Illegal)
                } else {
                    (addr, MemoryRegion::Arm7Bios)
                }
            }
            2 => ((addr & 0x3fffff), MemoryRegion::MainMemory),
            3 => {
                if !IS_ARM9 && addr >= 0x3800000 {
                    return (addr & 0xffff, MemoryRegion::Arm7Wram);
                }
                let bits = self.read_byte_raw(0x247, MemoryRegion::Arm9Io) & 0b11;
                let size = match (bits, IS_ARM9) {
                    (0b00, true) => 32 * 1024,
                    (0b00, false) => 0,
                    (0b01, _) | (0b10, _) => 16 * 1024,
                    (0b11, true) => 0,
                    (0b11, false) => 32 * 1024,
                    _ => unreachable!(),
                };
                if size == 32 * 1024 || (size == 0 && IS_ARM9) {
                    return (addr & (32 * 1024 - 1), MemoryRegion::SharedWram);
                }
                if size == 16 * 1024 {
                    if (bits == 0b01 && !IS_ARM9) || (bits == 0b10 && IS_ARM9) {
                        // first region
                        return (addr & (16 * 1024 - 1), MemoryRegion::SharedWram);
                    } else {
                        // second region
                        return (
                            (addr & (16 * 1024 - 1)) + 16 * 1024,
                            MemoryRegion::SharedWram,
                        );
                    }
                }
                assert!(size == 0 && !IS_ARM9);
                return (addr & 0xffff, MemoryRegion::Arm7Wram);
            }
            4 => {
                if !IS_ARM9 && addr > 0x4800000 {
                    // TODO
                    (addr, MemoryRegion::Arm7WirelessWaitstates)
                } else if addr >= 0x04000800 {
                    (addr, MemoryRegion::Illegal)
                } else {
                    // NOTE: not mirrored (maybe todo)
                    (
                        (addr & 0x7ff),
                        if IS_ARM9 {
                            MemoryRegion::Arm9Io
                        } else {
                            MemoryRegion::Arm7Io
                        },
                    )
                }
            }
            5 => {
                if IS_ARM9 {
                    ((addr & 0x7ff), MemoryRegion::Palette)
                } else {
                    // this region cannot be accessed by arm7 cpu
                    (addr, MemoryRegion::Illegal)
                }
            }
            6 => {
                if !is_read {
                    if let ChunkSize::Byte = chunk_size {
                        warn!("write byte to vram, not legal");
                        return (0, MemoryRegion::Illegal);
                    }
                }
                if IS_ARM9 {
                    match (addr >> 20) & 0b1111 {
                        0 | 1 => (addr & (0x80000 - 1), MemoryRegion::VramBgA),
                        2 | 3 => (addr & (0x20000 - 1), MemoryRegion::VramBgB),
                        4 | 5 => (addr & (0x40000 - 1), MemoryRegion::VramObjA),
                        6 | 7 => (addr & (0x20000 - 1), MemoryRegion::VramObjB),
                        _ => {
                            if addr > 0x06800000 + 656 * 1024 {
                                warn!("out of bounds access to lcdc vram");
                                (0, MemoryRegion::Illegal)
                            } else {
                                (addr - 0x06800000, MemoryRegion::VramLcdc)
                            }
                        }
                    }
                } else {
                    // vram shared as work memory to arm7, assume always turned ons
                    let new_addr = addr & (256 * 1024 - 1);
                    (new_addr + 256 * 1024, MemoryRegion::VramLcdc)
                }
            }
            7 => {
                if IS_ARM9 {
                    (addr & 0x7ff, MemoryRegion::Oam)
                } else {
                    warn!("arm7 attempted access to region 7, not defined");
                    (0, MemoryRegion::Illegal)
                }
            }
            0xff => {
                if IS_ARM9 && addr > 0xffff0000 {
                    let new_addr = addr - 0xffff0000;
                    if new_addr < 32 * 1024 {
                        return (new_addr, MemoryRegion::Arm9Bios);
                    }
                }
                warn!(
                    "failed attempt to access 0xff: is_arm9: {}, addr: {:#x}",
                    IS_ARM9, addr
                );
                (0, MemoryRegion::Illegal)
            }
            _ => {
                #[cfg(feature = "debug_instr")]
                warn!("illegal memory access: {:#x} {:#x}", addr, self.cpu.instr);
                (0, MemoryRegion::Illegal)
            }
        }
    }
}
