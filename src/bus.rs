pub struct Bus{
    mem: Vec<u8>
}

impl Bus {
    pub fn new() -> Bus{
        Bus { 
            mem: vec![0; 268435456]
        }
    }

    pub fn read_byte(&mut self, addr: usize) -> u8 {
        self.mem[addr]
    }

    pub fn read_halfword(&mut self, addr: usize) -> u16 {
        assert!(addr & 1 == 0);
        self.mem[addr] as u16 + ((self.mem[addr + 1] as u16) << 8)
    }

    pub fn read_word(&mut self, addr: usize) -> u32 {
        assert!(addr & 0b11 == 0);
        self.mem[addr] as u32 + ((self.mem[addr + 1] as u32) << 8) + ((self.mem[addr + 2] as u32) << 16) + ((self.mem[addr + 3] as u32) << 24)
    }

    pub fn store_byte(&mut self, addr: usize, val: u8) {
        self.mem[addr] = val;
    }

    pub fn store_halfword(&mut self, addr: usize, val: u16) {
        assert!(addr & 1 == 0);
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr + 1] = ((val >> 8) & 0b11111111) as u8;
    }

    pub fn store_word(&mut self, addr: usize, val: u32) {
        assert!(addr & 0b11 == 0);
        self.mem[addr] = (val & 0b11111111) as u8;
        self.mem[addr + 1] = ((val >> 8) & 0b11111111) as u8;
        self.mem[addr + 2] = ((val >> 16) & 0b11111111) as u8;
        self.mem[addr + 3] = ((val >> 24) & 0b11111111) as u8;
    }
}