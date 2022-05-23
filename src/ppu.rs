use crate::{
    bus::Bus,
};

use std::mem;

#[derive(Clone, Copy)]
pub enum Pixel {
    Colour(u8,u8,u8), // r, g, b
    Transparent,
}

impl Pixel{
    pub fn new_colour(r: u8, g: u8, b: u8) -> Pixel{
        assert!(r < 32 && g < 32 && b < 32);
        return Pixel::Colour(r,g,b)
    }

    pub fn to_float(&self) -> (f32, f32, f32) {
        if let &Pixel::Colour(r,g,b) = self{
            (r as f32 / 32., g as f32 / 32., b as f32 / 32.)
        }
        else{
            (0.,0.,0.)
        }
    }

    pub fn overwrite(&mut self, new_pixel: Pixel) {
        if let Pixel::Colour(r_old, g_old, b_old) = self{
            if let Pixel::Colour(r,g,b) = new_pixel{
                *r_old = r;
                *b_old = g;
                *g_old = b;
            }
        }
        else{
            panic!("overwritten pixel should not be transparent");
        }
    }
}

#[derive(Clone)]
pub struct ScreenBuffer {
    buffer: Vec<Vec<Pixel>>,
}

impl ScreenBuffer{
    pub fn new() -> ScreenBuffer{
        return ScreenBuffer{
            buffer: vec![vec![Pixel::new_colour(0,0,0); 240]; 160],
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

    cur_priority: u8,

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
            cur_scanline: [Pixel::new_colour(0,0,0); 240],

            cur_priority: 0,

            disp_cnt: 0,
            disp_stat: 0,
        
            cpu_interrupt: 0,
        }
    }

    pub fn check_cpu_interrupt(&mut self) -> u16 {
        let res = self.cpu_interrupt;
        self.cpu_interrupt = 0;
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
            let mut res = ScreenBuffer::new();
            mem::swap(&mut self.buffer, &mut res);
            Some(res)
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
        self.cur_scanline = [Pixel::new_colour(0,0,0); 240];
        for priority in (0..4).rev(){
            self.cur_priority = priority;
            // process background
            match self.disp_cnt & 0b111 {
                4 => self.process_bg_mode_4(bus),
                _ => {}
            }

            // process sprites
            self.process_sprites(bus);
        }
    }

    // -------- background processing methods

    fn process_bg_mode_4(&mut self, bus: &Bus) {
        // assume that one background of priority 3 is drawn
        if self.cur_priority < 3 {
            return;
        }
        let mut addr = 0x06000000 + self.cur_line as usize * 240;

        // frame number
        if (self.disp_cnt >> 4) & 1 > 0 {
            addr += 0x9600;
        }

        for i in 0..240 {
            self.cur_scanline[i].overwrite(self.process_palette_colour(bus.read_byte(addr + i), false, bus));
        }
    }

    // -------- sprite processing
    fn process_sprites(&mut self, bus: &Bus) {
        let map_mode = (self.disp_cnt >> 6) & 1 > 0; // 0 means 2D mapping. 1 means 1D mapping. 
        let base_oam_addr = 0x7000000;

        for k in (0..128).rev() {
            // process sprite attributes
            let attr0 = bus.read_halfword(base_oam_addr + k * 8);
            let obj_mode = (attr0 >> 8) & 0b11;
            if obj_mode == 0b10 {
                // no rendering
                continue;
            }
            let attr2 = bus.read_halfword(base_oam_addr + k * 8 + 4);
            let cur_p = ((attr2 >> 10) & 0b11) as u8;
            if cur_p != self.cur_priority{
                continue;
            }

            let density = (attr0 >> 13) & 1 > 0; // 0 means 4 bits per pixel, 1 means 8 bits per pixel
            let tile_size = if density {
                64
            }
            else{
                32
            };
            
            let attr1 = bus.read_halfword(base_oam_addr + k * 8 + 2);
            let base_tile_index = attr2 & 0b1111111111;
            if self.disp_cnt & 0b111 >= 3 && base_tile_index < 512 {
                continue; // ignore lower charblock on bitmap modes
            }
            let addr = base_tile_index as usize * 32 + 0x6010000;

            let y = attr0 & 0b11111111;
            let x = attr1 & 0b11111111;

            // width, height
            let (w,h) = self.get_sprite_dimensions((attr0 >> 14) as u8, (attr1 >> 14) as u8);
            let row_size = tile_size as usize * w as usize;

            for i in 0..h as usize{
                // todo: consider 2d mapping
                let row = bus.bulk_read_word(addr + row_size * i, row_size);
                for j in 0..w as usize{
                    
                    let pal = 
                    // 4 bits per pixel
                    if !density { 
                        if j & 1 > 0{
                            row[j >> 1] >> 4
                        }
                        else{
                            row[j >> 1]
                        }
                    }
                    // 8 bits per pixel
                    else{
                        row[j]
                    };
                    let pixel = self.process_palette_colour(pal, true, bus);

                    // TODO: process affine transformations
                    if i as u8 + y as u8 == self.cur_line && (j + x as usize) < 240{
                        self.cur_scanline[j + x as usize].overwrite(pixel);
                    }
                }
            }
        }
        
    }

    // returns width, height
    fn get_sprite_dimensions(&self, shape: u8, size: u8) -> (u8, u8) {
        match(shape, size) {
            (0b00, 0b00) => (8,8),
            (0b00, 0b01) => (16,16),
            (0b00, 0b10) => (32,32),
            (0b00, 0b11) => (64,64),
            (0b01, 0b00) => (16,8),
            (0b01, 0b01) => (32,8),
            (0b01, 0b10) => (32,16),
            (0b01, 0b11) => (64,32),
            (0b10, 0b00) => (8,16),
            (0b10, 0b01) => (8,32),
            (0b10, 0b10) => (16,32),
            (0b10, 0b11) => (32,64),
            _ => panic!("invalid sprite shape and/or size")
        }
    }

    // ------- helper functions

    fn process_15bit_colour(&self, halfword: u16) -> Pixel {
        Pixel::new_colour((halfword & 0b11111) as u8, ((halfword >> 5) & 0b11111) as u8, ((halfword >> 10) & 0b11111) as u8)
    }

    fn process_palette_colour(&self, palette_index: u8, is_sprite: bool, bus: &Bus) -> Pixel {
        let mut addr = 0x05000000 + palette_index as u32 * 2;
        if is_sprite{
            addr += 0x200;
        }
        let index = bus.read_halfword(addr as usize);
        match index{
            0 => Pixel::Transparent,
            _ => self.process_15bit_colour(index)
        } 
        
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