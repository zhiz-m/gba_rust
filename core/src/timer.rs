use crate::{
    bus::{Bus, MemoryRegion},
    config,
};

pub struct Timer {
    timer_no: u8,
    pub timer_count: u16,
    cur_cycle: u16,
    period: u16,
    period_pow: u16,
    pub reload_val: u16,
    pub raise_interrupt: bool,
    pub is_cascading: bool,
    pub is_enabled: bool,
    //direct_sound_channel: Option<usize>,
}

impl Timer {
    pub fn new(timer_no: u8) -> Timer {
        Timer {
            timer_no,
            timer_count: 0,
            cur_cycle: 0,
            period: 1,
            period_pow: 0,
            reload_val: 0,
            raise_interrupt: false,
            is_cascading: false,
            is_enabled: false,
            //direct_sound_channel: None,
        }
    }

    // bits must be: [0, 4)
    #[inline(always)]
    pub fn set_period(&mut self, bits: u8) {
        self.period_pow = match bits {
            0b00 => 0,
            0b01 => 6,
            0b10 => 8,
            0b11 => 10,
            _ => unreachable!("timer invalid period"),
        };
        self.period = 1 << self.period_pow;
        //info!("timer: {}, period: {}", self.timer_no, self.period);
    }

    pub fn sync_registers_to_bus(&self, bus: &mut Bus) {
        let addr = 0x100 + ((self.timer_no as usize) << 2);
        bus.store_byte_raw(addr, MemoryRegion::IO, self.timer_count as u8);
        bus.store_byte_raw(addr + 1, MemoryRegion::IO, (self.timer_count >> 8) as u8);
    }

    #[inline(always)]
    pub fn set_is_enabled(&mut self, bus: &mut Bus, enable: bool) {
        //info!("timer_no: {}, enabled: {}", self.timer_no, enable);
        if enable && !self.is_enabled {
            self.timer_count = self.reload_val;
            self.sync_registers_to_bus(bus);
        } else if !enable && self.is_enabled {
        }
        self.is_enabled = enable;
    }

    // returns true if overflow happened
    #[inline(always)]
    pub fn clock(&mut self, bus: &mut Bus) -> bool {
        if !self.is_cascading {
            self.cur_cycle += config::TIMER_CLOCK_INTERVAL_CLOCKS as u16;
        }

        if self.cur_cycle >= self.period {
            let timer_count_old = self.timer_count;
            self.timer_count += self.cur_cycle >> self.period_pow;
            self.cur_cycle &= self.period - 1;
            self.sync_registers_to_bus(bus);

            // overflow
            if self.timer_count < timer_count_old {
                //info!("timer_no: {}, reload_val: {}, period: {}", self.timer_no, self.reload_val, self.period);
                // increment the position of next Direct Sound sample played
                //let snd_ds_cnt = bus.read_halfword_raw(0x04000082);
                for i in 0..2 {
                    /*let enable_right_left = [(snd_ds_cnt >> (8 + 4 * i)) & 1 > 0, (snd_ds_cnt >> (9 + 4 * i)) & 1 > 0];
                    if !enable_right_left[0] && !enable_right_left[1] {
                        continue;
                    }*/
                    if let Some(timer_no) = bus.apu.direct_sound_timer[i] {
                        if timer_no == self.timer_no as usize {
                            //bus.apu.direct_sound_fifo_cur[0] = *bus.apu.direct_sound_fifo[0].front().unwrap();
                            if let Some(val) = bus.apu.direct_sound_fifo[i].pop_front() {
                                bus.apu.direct_sound_fifo_cur[i] = val;
                            } else {
                                //warn!("timer overflow; attempted read from empty fifo")
                            }
                        }
                    }
                }

                /*if let Some(timer_no) = bus.apu.direct_sound_timer[1] {
                    if timer_no == self.timer_no{
                        if let Some(val) = bus.apu.direct_sound_fifo[1].pop_front(){
                            bus.apu.direct_sound_fifo_cur[1] = val;
                        }
                    }
                }*/
                self.timer_count += self.reload_val;
                self.sync_registers_to_bus(bus);
                if self.raise_interrupt {
                    bus.cpu_interrupt(1 << (3 + self.timer_no));
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn cascade(&mut self) {
        assert!(self.is_cascading);
        self.cur_cycle += 1;
    }
}
