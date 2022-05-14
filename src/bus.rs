pub struct Bus{
    mem: [u8; 268435456]
}

impl Bus {
    pub fn new() -> Bus{
        Bus { 
            mem: [0; 268435456]
        }
    }

    pub fn read_byte(&mut self, addr: usize) -> u8 {
        self.mem[addr]
    }

    pub fn read_halfword(&mut self, addr: usize) -> u16 {
        self.mem[addr] as u16 + ((self.mem[addr + 8] as u16) << 8)
    }

    pub fn read_word(&mut self, addr: usize) -> u32 {
        self.mem[addr] as u32 + ((self.mem[addr + 8] as u32) << 8) + ((self.mem[addr + 16] as u32) << 16) + ((self.mem[addr + 24] as u32) << 24)
    }
}