use super::{
    frontend::{
        ScreenBuffer, Pixel
    },
    bus::Bus,
};

pub struct PPU {
    clock_total: u64,
    clock_cur: u32,

    buffer: ScreenBuffer,
}

impl PPU {
    pub fn new() -> PPU {
        PPU{
            clock_total: 0,
            clock_cur: 0,

            buffer: ScreenBuffer::new(),
        }
    }

    pub fn clock(&mut self, bus: &mut Bus) -> Option<&ScreenBuffer> {
        let mut res = None;
        if self.clock_cur == 0{
            self.clock_cur += self._clock(bus);
            res = Some(&self.buffer);
        }
        res
    }

    // returns number of clock cycles
    fn _clock(&mut self, bus: &Bus) -> u32 {
        let mut i = 0;
        let mut j = 0;
        let mut addr = 0x06000000;
        let mut res = 0;
        let mut num_bits_res = 0;
        while i < 160 && j < 240 {
            if 15 > num_bits_res {
                res = res | ((bus.read_halfword(addr) as u32) << num_bits_res);
                num_bits_res += 16;
                addr += 2;
            }
            else{
                let pixel = Pixel::new((res & 0b11111) as f32 / 31., ((res >> 5) & 0b11111) as f32 / 31., ((res >> 10) & 0b11111) as f32 / 31.);
                self.buffer.write_pixel(i, j, pixel);

                res >>= 15;
                num_bits_res -= 15;

                j += 1;
                if j == 240 {
                    j = 0;
                    i += 1;
                }
            }
            

        };

        100
    }
}