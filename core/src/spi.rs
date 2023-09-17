use log::warn;

use crate::bus::{Bus, MemoryRegion};

// much of the implementation comes from CorgiDs, https://github.com/PSI-Rockin/CorgiDS
enum FirmwareCommand{
    ReadData,
    ReadStatus,
    None
}

struct Firmware{
    firmware: Box<[u8; 0x4000]>,
    command: FirmwareCommand,
    read_addr: u32,
    addr_byte_cnt: u8,
    status: u8,
}

impl Default for Firmware{
    fn default() -> Self {
        let mut firmware = Box::new([0u8; 0x4000]);
        
        // credit: shonumi 
        //Header - Console Type - Original NDS
        firmware[0x1D] = 0xFF;
        firmware[0x1E] = 0xFF;
        firmware[0x1F] = 0xFF;

        //Header - User settings offset - 0x3FE00 aka User Settings Area 1
        firmware[0x20] = 0xC0;
        firmware[0x21] = 0x7F;

        //Wifi Config Length
        firmware[0x2C] = 0x38;
        firmware[0x2D] = 0x01;

        //Wifi list of enabled channels (1-13)
        firmware[0x3C] = 0xFE;
        firmware[0x3D] = 0x3F;

        //User Settings Area 1 - Version - Always 0x5
        firmware[0x3FE00] = 0x5;

        //User Settings Area 1 - Favorite Color - We like blue
        firmware[0x3FE02] = 0xB;

        //User Settings Area 1 - Our bday is April 1st
        firmware[0x3FE03] = 0x4;
        firmware[0x3FE04] = 0x1;

        //User Settings Area 1 - Nickname - GBE+
        firmware[0x3FE06] = 0x47;
        firmware[0x3FE08] = 0x42;
        firmware[0x3FE0A] = 0x45;
        firmware[0x3FE0C] = 0x2B;

        //User Settings Area 1 - Touchscreen calibration points
        firmware[0x3FE58] = 0xA4;
        firmware[0x3FE59] = 0x02;
        firmware[0x3FE5A] = 0xF4;
        firmware[0x3FE5B] = 0x02;
        firmware[0x3FE5C] = 0x20;
        firmware[0x3FE5D] = 0x20;
        firmware[0x3FE5E] = 0x24;
        firmware[0x3FE5F] = 0x0D;
        firmware[0x3FE60] = 0xE0;
        firmware[0x3FE61] = 0x0C;
        firmware[0x3FE62] = 0xE0;
        firmware[0x3FE63] = 0xA0;

        //User Settings Area 1 - Language Flags
        firmware[0x3FE64] = 0x01;
        firmware[0x3FE65] = 0xFC;

        //User Settings Area 1 - Nickname length
        firmware[0x3FE1A] = 0x4;

        //User Settings CRC16
        firmware[0x3FE72] = 0x42;
        firmware[0x3FE73] = 0x1A;

        //Copy User Settings 0 to User Settings 1
        // for(u32 x = 0; x < 0x100; x++)
        for x in 0..0x100
        {
            firmware[0x3FF00 + x] = firmware[0x3FE00 + x];
        }

        //Set Update Counter for User Settings 1 higher than User Settings 0
        firmware[0x3FF70] = firmware[0x3FE70] + 1;
    
        // TODO: touch screen calibration

        Self{firmware, command: FirmwareCommand::None, read_addr: 0, addr_byte_cnt: 0, status: 0}
    }
}

impl Firmware{
    fn clear_commands(&mut self){
        self.command = FirmwareCommand::None;
        self.read_addr = 0;
        self.addr_byte_cnt = 0;
    }

    fn transfer(&mut self, input: u8) -> u8{
        match self.command{
            FirmwareCommand::ReadStatus => self.status,
            FirmwareCommand::ReadData => {
                if self.addr_byte_cnt < 3{
                    self.read_addr <<= 8;
                    self.read_addr |= input as u32;
                    self.addr_byte_cnt += 1;
                    input
                }
                else{
                    let res = self.firmware[self.read_addr as usize];
                    self.read_addr += 1;
                    res
                }
            }
            FirmwareCommand::None => {
                match input{
                    0x03 => self.command = FirmwareCommand::ReadData,
                    0x04 => self.status &= !1, // disable writes
                    0x05 => self.command = FirmwareCommand::ReadStatus,
                    0x06 => self.status |= 1, // enable writes
                    0x0A => warn!("firmware write command requested, but is not implemented"),
                    x => warn!("unknown firmware input: {}", x),
                }
                input
            }
        }
    }
}

// todo
struct Touchscreen{

}

impl Default for Touchscreen{
    fn default() -> Self {
        Self {  }
    }
}

impl Touchscreen{
    // TODO
    fn transfer(&mut self, input: u8) -> u8{
        warn!("touchscreen transfer not implemented");
        0
    }
}

pub struct Spi{
    firmware: Firmware,
    touchscreen: Touchscreen,
    pub spi_cnt: u16,
    output: u8,
}

impl Default for Spi{
    fn default() -> Self {
        Self { firmware: Default::default(), touchscreen: Default::default(), spi_cnt: 0, output: 0 }
    }
}

impl Spi{
    pub fn write_data(&mut self, input: u8, bus: &mut Bus) {
        // let mut spi_cnt = bus.read_halfword_raw(0x1C0, MemoryRegion::Arm7Io);
        if self.spi_cnt >> 15 == 0{
            return;
        }

        // set busy flag to 0
        self.spi_cnt &= !0b10000000;
    
        match (self.spi_cnt >> 8) & 0b11{
            0b00 => self.output = 0, // ignore power manager
            0b01 => {
                self.output = self.firmware.transfer(input);
                if (self.spi_cnt >> 11) & 1 > 0{
                    self.firmware.clear_commands();
                }
            }
            0b10 => {
                self.output = self.touchscreen.transfer(input);
            }
            0b11 => {
                self.output = 0;
                warn!("reserved spi device not implemented / provided");
            }
            _ => unreachable!()
        }

        if (self.spi_cnt >> 14) & 1 > 0{
            bus.cpu_interrupt::<false>(1 << 23);
        }

        // bus.store_halfword_raw(0x1C0, MemoryRegion::Arm7Io, spi_cnt);
    }

    pub fn read_data(&self) -> u8 {
        if self.spi_cnt >> 15 == 0{
            0
        }
        else{
            self.output
        }
    }
}