use crate::bus::Bus;

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
        };
        //println!("timer: {}, freq: {}", self.timer_no, self.freq);
    }

    // assumes timer is not enabled yet
    pub fn set_is_enabled(&mut self, enable: bool) {
        if enable && !self.is_enabled{
            self.timer_count = self.reload_val;
            //println!("timer: {}, reload: {}", self.timer_no, self.reload_val);
        }
        self.is_enabled = enable;
    }

    // returns true if overflow happened
    pub fn clock(&mut self, bus: &mut Bus) -> bool {
        if !self.is_cascading {
            self.cur_cycle += 1;
        }

        if self.cur_cycle >= self.freq {
            self.cur_cycle = 0;
            self.timer_count += 1;
            if self.timer_count == 0 {
                self.timer_count = self.reload_val;
                if self.raise_interrupt{
                    bus.cpu_interrupt(1 << (3 + self.timer_no));
                }
                true
            }
            else{
                false
            }
        }
        else{
            false
        }
    }

    pub fn cascade(&mut self) {
        assert!(self.is_cascading);
        self.cur_cycle += 1;
    }
}