use crate::{
    bus::Bus,
};

#[derive(Clone, Copy)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel{
    pub fn new(r: u8, g: u8, b: u8) -> Pixel{
        assert!(r < 32 && g < 32 && b < 32);
        return Pixel { r, g, b }
    }

    pub fn to_float(&self) -> (f32, f32, f32) {
        (self.r as f32 / 32., self.g as f32 / 32., self.b as f32 / 32.)
    }
}

#[derive(Clone)]
pub struct ScreenBuffer {
    buffer: Vec<Vec<Pixel>>,
}

impl ScreenBuffer{
    pub fn new() -> ScreenBuffer{
        return ScreenBuffer{
            buffer: vec![vec![Pixel::new(0,0,0); 240]; 160],
        }
    }
    pub fn write_pixel(&mut self, row: usize, col: usize, pixel: Pixel){
        self.buffer[row][col] = pixel;
    }
    pub fn read_pixel(&self, row: usize, col: usize) -> Pixel{
        return self.buffer[row][col];
    }
}


pub struct PPU {
    clock_cur: u32,

    buffer: ScreenBuffer,
    buffer_ready: bool,

    is_hblank: bool,
    cur_line: u8, // current line being processed. 
    cur_scanline: [Pixel; 240],

    disp_cnt: u16,
    disp_stat: u16,

    cpu_interrupt: u16,
}

impl PPU {
    pub fn new() -> PPU {
        PPU{
            clock_cur: 960, // clocks needed to process first scanline

            buffer: ScreenBuffer::new(),
            buffer_ready: false,

            is_hblank: false,
            cur_line: 0,
            cur_scanline: [Pixel::new(0,0,0); 240],

            disp_cnt: 0,
            disp_stat: 0,
        
            cpu_interrupt: 0,
        }
    }

    pub fn check_cpu_interrupt(&mut self) -> u16 {
        let res = self.cpu_interrupt;
        self.cpu_interrupt = 0;
        if res > 0{
            //println!("ppu cpu_interrupt: {:#018b}", res);
        }
        res
    }

    pub fn clock(&mut self, bus: &mut Bus) -> Option<ScreenBuffer> {
        self.buffer_ready = false;

        // may clock more than once per call to this function
        // only happens when transitioning to vblank
        if self.clock_cur == 0{
            self.clock_cur += self._clock(bus);
        }

        assert!(self.clock_cur > 0);
        self.clock_cur -= 1;

        if self.buffer_ready{
            Some(self.buffer.clone())
        }
        else{
            None
        }
    }

    fn _clock(&mut self, bus: &mut Bus) -> u32 {
        self.disp_cnt = bus.read_halfword(0x04000000);
        self.disp_stat = bus.read_halfword(0x04000004);

        let res = 

        if self.cur_line >= 160 {
            self.cur_line += 1;
            if self.cur_line == 228 {
                self.is_hblank = false;
                self.cur_line = 0;
                960
            }
            else{
                1232
            }
        }
        else if !self.is_hblank {
            self.process_scanline(bus);
            for j in 0..240 {
                self.buffer.write_pixel(self.cur_line as usize, j, self.cur_scanline[j]);
            }
            //println!("  scanline processed: {}", self.cur_line);

            self.is_hblank = true;

            // set hblank interrupt
            if (self.disp_stat >> 4) & 1 > 0 {
                self.cpu_interrupt |= 0b10;
            }

            272
        }
        else{
            self.is_hblank = false;
            self.cur_line += 1;

            if self.cur_line == 160{
                self.buffer_ready = true;
                1232
            }
            else{
                960
            }
        };
        // store VCOUNT
        bus.store_byte(0x04000006, self.cur_line);

        self.disp_stat &= !0b111;
        if self.cur_line >= 160 {
            // set vblank interrupt
            if self.cur_line == 160 && (self.disp_stat >> 3) & 1 > 0 {
                self.cpu_interrupt |= 1;
            }
            self.disp_stat |= 0b001;
        }
        if self.is_hblank{
            self.disp_stat |= 0b010;
        }
        // vcount interrupt request
        if (self.disp_stat >> 5) & 1 > 0 && self.cur_line as u16 == self.disp_stat >> 8{
            self.disp_stat |= 0b100;
            self.cpu_interrupt |= 0b100;
        }

        bus.store_halfword(0x04000004, self.disp_stat);

        res
    }

    fn process_scanline(&mut self, bus: &Bus) {
        match self.disp_cnt & 0b111 {
            4 => self.process_bg_mode_4(bus),
            _ => {}
        }
    }

    fn process_bg_mode_4(&mut self, bus: &Bus) {
        let mut addr = 0x06000000 + self.cur_line as usize * 240;

        // frame number
        if (self.disp_cnt >> 4) & 1 > 0 {
            addr += 0x9600;
        }

        for i in 0..240 {
            self.cur_scanline[i] = self.process_palette_colour(bus.read_byte(addr + i), false, bus);
        }
    }

    // ------- helper functions

    fn process_15bit_colour(&self, halfword: u16) -> Pixel {
        Pixel::new((halfword & 0b11111) as u8, ((halfword >> 5) & 0b11111) as u8, ((halfword >> 10) & 0b11111) as u8)
    }

    fn process_palette_colour(&self, palette_no: u8, is_sprite: bool, bus: &Bus) -> Pixel {
        let mut addr = 0x05000000 + palette_no as u32 * 2;
        if is_sprite{
            addr += 0x200;
        }
        self.process_15bit_colour(bus.read_halfword(addr as usize))
    }

    /*
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
    */
}