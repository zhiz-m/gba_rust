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

    direct_sound_channel: Option<usize>,
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
            
            direct_sound_channel: None,
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

    pub fn set_is_enabled(&mut self, enable: bool) {
        if enable && !self.is_enabled{
            self.timer_count = self.reload_val;
        }
        else if !enable && self.is_enabled{

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
                // increment the position of next Direct Sound sample played
                if let Some(timer_no) = bus.apu.direct_sound_timer[0] {
                    if timer_no == self.timer_no{
                        //bus.apu.direct_sound_fifo_cur[0] = *bus.apu.direct_sound_fifo[0].front().unwrap();
                        if let Some(val) = bus.apu.direct_sound_fifo[0].pop_front(){
                            bus.apu.direct_sound_fifo_cur[0] = val;
                        }
                        else{
                            //println!("timer overflow; attempted read from empty fifo")
                        }
                    }
                }
                if let Some(timer_no) = bus.apu.direct_sound_timer[1] {
                    if timer_no == self.timer_no{
                        if let Some(val) = bus.apu.direct_sound_fifo[1].pop_front(){
                            bus.apu.direct_sound_fifo_cur[1] = val;
                        }
                    }
                }
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