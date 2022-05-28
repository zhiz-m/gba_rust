pub struct Timer {
    timer_no: usize,
    pub timer_count: u16,
    cur_cycle: u16,
    freq: u16,
    pub reload_val: u16,
    pub raise_interrupt: bool,
    pub is_cascading: bool,
    pub is_enabled: bool,
}

impl Timer{
    pub fn new(timer_no: usize) -> Timer{
        Timer {
            timer_no,
            timer_count: 0,
            cur_cycle: 0,
            freq: 1,
            reload_val: 0,
            raise_interrupt: false,
            is_cascading: false,
            is_enabled: false,
        }
    }

    // bits must be: [0, 4]
    pub fn set_frequency(&mut self, bits: u8) {
        self.freq = match bits {
            0b00 => 1,
            0b01 => 64,
            0b10 => 256,
            0b11 => 1024,
            _ => panic!("timer invalid frequency")
        }
    }
}