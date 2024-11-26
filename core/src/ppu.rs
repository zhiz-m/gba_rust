#![allow(non_camel_case_types)]

use log::warn;

use crate::bus::{Bus, MemoryRegion};

use std::num::Wrapping;

#[derive(Clone, Copy)]
pub struct Pixel(u8, u8, u8);

impl Pixel {
    pub fn new(r: u8, g: u8, b: u8) -> Pixel {
        Pixel(r.min(31), g.min(31), b.min(31))
    }

    pub fn to_float(&self) -> (f32, f32, f32) {
        (
            self.0 as f32 / 31.,
            self.1 as f32 / 31.,
            self.2 as f32 / 31.,
        )
    }

    pub fn to_u8(&self) -> (u8, u8, u8) {
        (self.0 << 3, self.1 << 3, self.2 << 3)
    }

    pub fn blend(pixel_front: Pixel, pixel_back: Pixel, a: u16, b: u16) -> Pixel {
        Pixel::new(
            ((pixel_front.0 as u16 * a + pixel_back.0 as u16 * b) >> 4) as u8,
            ((pixel_front.1 as u16 * a + pixel_back.1 as u16 * b) >> 4) as u8,
            ((pixel_front.2 as u16 * a + pixel_back.2 as u16 * b) >> 4) as u8,
        )
    }
}

pub trait RenderOutput {
    fn set(&mut self, y: usize, scanline: &[Pixel; 240]);
}

#[derive(Clone)]
pub struct ScreenBuffer {
    buffer: Box<[[Pixel; 240]; 160]>,
}

impl Default for ScreenBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenBuffer {
    pub fn new() -> ScreenBuffer {
        ScreenBuffer {
            buffer: Box::new([[Pixel::new(0, 0, 0); 240]; 160]),
        }
    }
    pub fn write_pixel(&mut self, row: usize, col: usize, pixel: Pixel) {
        self.buffer[row][col] = pixel;
    }
    pub fn read_pixel(&self, row: usize, col: usize) -> Pixel {
        self.buffer[row][col]
    }
}

#[derive(PartialEq, Clone, Copy)]
enum WindowType {
    W_0 = 0,
    W_1 = 1,
    W_obj = 2,
    W_out = 3,
    W_full = 4, // W_full is used when there are no windows active
}

#[derive(PartialEq, Clone, Copy)]
enum PixelType {
    BG_0 = 0,
    BG_1 = 1,
    BG_2 = 2,
    BG_3 = 3,
    Sprite = 4,
    Backdrop = 5,
    Sprite_blend = 6,
}

pub struct Ppu {
    //clock_cur: u32,
    buffer: ScreenBuffer,
    pub buffer_ready: bool,

    is_hblank: bool,
    cur_line: u8, // current line being processed.
    cur_scanline: Vec<Pixel>,
    cur_scanline_front: Vec<(Pixel, PixelType, WindowType)>,
    cur_scanline_back: Vec<(Pixel, PixelType, WindowType)>,

    window_scanlines: [Vec<bool>; 4],
    active_windows: [bool; 4],
    window_flags: [u8; 4],
    is_windowing_active: bool,
    cur_window: WindowType,

    cur_priority: u8,

    disp_cnt: u16,
    disp_stat: u16,

    cpu_interrupt: u16,

    frame_count: u32,
    pub frame_count_render: u32,
}

impl Ppu {
    pub fn new() -> Ppu {
        Ppu {
            //clock_cur: 960, // clocks needed to process first scanline
            buffer: ScreenBuffer::new(),
            buffer_ready: false,

            is_hblank: false,
            cur_line: 0,
            cur_scanline: vec![Pixel::new(0, 0, 0); 240],
            cur_scanline_front: vec![
                (Pixel::new(0, 0, 0), PixelType::Backdrop, WindowType::W_full);
                240
            ],
            cur_scanline_back: vec![
                (Pixel::new(0, 0, 0), PixelType::Backdrop, WindowType::W_full);
                240
            ],

            window_scanlines: std::array::from_fn(|_| vec![true; 240]),
            active_windows: [false; 4],
            window_flags: [0; 4],
            is_windowing_active: false,
            cur_window: WindowType::W_full,

            cur_priority: 0,

            disp_cnt: 0,
            disp_stat: 0,

            cpu_interrupt: 0,

            frame_count: 0,
            frame_count_render: 1,
        }
    }

    pub fn get_screen_buffer(&mut self) -> Option<&ScreenBuffer> {
        if self.buffer_ready {
            self.buffer_ready = false;
            Some(&self.buffer)
        } else {
            None
        }
    }

    pub fn clock(&mut self, bus: &mut Bus) -> u32 {
        self._clock(bus)
    }

    fn _clock(&mut self, bus: &mut Bus) -> u32 {
        self.disp_cnt = bus.read_halfword_raw(0x0, MemoryRegion::IO);
        self.disp_stat = bus.read_halfword_raw(0x4, MemoryRegion::IO);

        let res = if self.cur_line >= 160 {
            self.cur_line += 1;
            if self.cur_line == 228 {
                self.is_hblank = false;
                self.cur_line = 0;
                960
            } else {
                1232
            }
        } else if !self.is_hblank {
            if self.frame_count == 0 {
                self.process_scanline(bus);
                for j in 0..240 {
                    self.buffer
                        .write_pixel(self.cur_line as usize, j, self.cur_scanline[j]);
                }
            }
            //info!("  scanline processed: {}", self.cur_line);

            self.is_hblank = true;

            // set hblank interrupt, dma
            if (self.disp_stat >> 4) & 1 > 0 {
                self.cpu_interrupt |= 0b10;
            }
            bus.hblank_dma = true;

            272
        } else {
            self.is_hblank = false;
            self.cur_line += 1;

            if self.cur_line == 160 {
                if self.frame_count == 0 {
                    self.buffer_ready = true;
                }
                self.frame_count += 1;
                if self.frame_count >= self.frame_count_render {
                    self.frame_count = 0;
                }
                1232
            } else {
                960
            }
        };
        // store VCOUNT
        bus.store_byte_raw(0x6, MemoryRegion::IO, self.cur_line);

        self.disp_stat &= !0b111;
        if self.cur_line >= 160 {
            // set vblank interrupt
            if self.cur_line == 160 {
                if (self.disp_stat >> 3) & 1 > 0 {
                    self.cpu_interrupt |= 1;
                }
                bus.vblank_dma = true;
            }
            self.disp_stat |= 0b001;
        }
        if self.is_hblank {
            self.disp_stat |= 0b010;
        }
        // vcount interrupt request
        if !self.is_hblank && self.cur_line as u16 == (self.disp_stat >> 8) {
            if (self.disp_stat >> 5) & 1 > 0 {
                self.cpu_interrupt |= 0b100;
                //info!("vcount irq requested: {}, frame: {}", self.disp_stat >> 8, self.frame_count);
            }
            self.disp_stat |= 0b100;
        }

        bus.store_halfword_raw(0x4, MemoryRegion::IO, self.disp_stat);
        if self.cpu_interrupt > 0 {
            bus.cpu_interrupt(self.cpu_interrupt);
            self.cpu_interrupt = 0;
        }

        res
    }

    fn process_scanline(&mut self, bus: &Bus) {
        let backdrop_colour = bus.read_halfword_raw(0x0, MemoryRegion::Palette);
        //self.cur_scanline.iter_mut().for_each(|x| *x = PPU::process_15bit_colour(backdrop_colour));
        self.cur_scanline_back.iter_mut().for_each(|x| {
            *x = (
                Ppu::process_15bit_colour(backdrop_colour),
                PixelType::Backdrop,
                WindowType::W_full,
            )
        });
        self.cur_scanline_front.iter_mut().for_each(|x| {
            *x = (
                Ppu::process_15bit_colour(backdrop_colour),
                PixelType::Backdrop,
                WindowType::W_full,
            )
        });

        self.init_window_scanline(bus);

        //info!("bldcnt: {:#018b}, bldalpha: {:#018b}", bus.read_halfword_raw(0x04000050), bus.read_halfword_raw(0x04000052));
        //info!("bg0cnt: {:#018b}, bg2cnt: {:#018b}", bus.read_halfword_raw(0x04000008), bus.read_halfword_raw(0x04000012));

        for win in [
            WindowType::W_full,
            WindowType::W_obj,
            WindowType::W_out,
            WindowType::W_1,
            WindowType::W_0,
        ] {
            if (win == WindowType::W_full) == (self.is_windowing_active)
                || (win as u16) < 4 && !self.active_windows[win as usize]
            {
                continue;
            }
            self.cur_window = win;
            for priority in (0..4).rev() {
                self.cur_priority = priority;
                // process background
                match self.disp_cnt & 0b111 {
                    0 => {
                        self.process_tiled_bg(PixelType::BG_3, false, bus);
                        self.process_tiled_bg(PixelType::BG_2, false, bus);
                        self.process_tiled_bg(PixelType::BG_1, false, bus);
                        self.process_tiled_bg(PixelType::BG_0, false, bus);
                    }
                    1 => {
                        self.process_tiled_bg(PixelType::BG_2, true, bus);
                        self.process_tiled_bg(PixelType::BG_1, false, bus);
                        self.process_tiled_bg(PixelType::BG_0, false, bus);
                    }
                    2 => {
                        self.process_tiled_bg(PixelType::BG_3, true, bus);
                        self.process_tiled_bg(PixelType::BG_2, true, bus);
                    }
                    3 => self.process_bg_mode_3(bus),
                    4 => self.process_bg_mode_4(bus),
                    5 => warn!("current bg mode 5, not implemented yet"),
                    _ => {}
                }

                // process sprites
                //if self.cur_window != WindowType::W_obj{
                self.process_sprites(false, bus);
                //}
            }
        }

        // process blending; update self.cur_scanline

        let bld_cnt = bus.read_halfword_raw(0x50, MemoryRegion::IO);
        let bld_alpha = bus.read_halfword_raw(0x52, MemoryRegion::IO);
        let bw_fade = bus.read_halfword_raw(0x54, MemoryRegion::IO) & 0b11111;
        let bm = (bld_cnt >> 6) & 0b11;
        let eva = bld_alpha & 0b11111;
        let evb = (bld_alpha >> 8) & 0b11111;
        //info!("eva: {:#07b}, evb: {:#07b}, bw_fade: {:#07b}", eva, evb, bw_fade);

        for i in 0..240 {
            let (pixel1, mut pixel_type1, win) = self.cur_scanline_front[i];

            let cur_bm = if pixel_type1 == PixelType::Sprite_blend {
                pixel_type1 = PixelType::Sprite;
                0b01
            } else {
                bm
            };
            //if win == WindowType::W_full {
            //    assert !(!self.is_windowing_active);
            //}
            if cur_bm == 0
                || pixel_type1 == PixelType::Backdrop
                || (bld_cnt >> pixel_type1 as u16) & 1 == 0
                || (self.is_windowing_active && (self.window_flags[win as usize] >> 5) & 1 == 0)
            {
                self.cur_scanline[i] = pixel1;
                continue;
            }

            match cur_bm {
                0b10 => {
                    self.cur_scanline[i] =
                        Pixel::blend(pixel1, Pixel::new(31, 31, 31), 0b10000 - bw_fade, bw_fade);
                    continue;
                }
                0b11 => {
                    self.cur_scanline[i] =
                        Pixel::blend(pixel1, Pixel::new(0, 0, 0), 0b10000 - bw_fade, bw_fade);
                    continue;
                }
                _ => {}
            }

            let (pixel2, pixel_type2, _) = self.cur_scanline_back[i];
            if (bld_cnt >> (pixel_type2 as u16 + 8)) & 1 == 0 {
                self.cur_scanline[i] = pixel1;
                continue;
            }
            self.cur_scanline[i] = Pixel::blend(pixel1, pixel2, eva, evb);
        }
    }

    // -------- background processing methods

    fn process_bg_mode_3(&mut self, bus: &Bus) {
        // assume that one background of priority 3 is drawn
        if !self.check_window_bg(PixelType::BG_0) || self.cur_priority < 3 {
            return;
        }
        let addr = self.cur_line as usize * 240 * 2;

        for i in 0..240 {
            self.update_cur_scanline_bg(
                i,
                Some(Ppu::process_15bit_colour(
                    bus.read_halfword_raw(addr + i * 2, MemoryRegion::Vram),
                )),
                PixelType::BG_0,
            );
        }
    }

    fn process_bg_mode_4(&mut self, bus: &Bus) {
        // assume that one background of priority 3 is drawn
        if self.cur_priority < 3 {
            return;
        }
        let mut addr = self.cur_line as usize * 240;

        let pixel_type;

        // frame number
        if (self.disp_cnt >> 4) & 1 > 0 {
            pixel_type = PixelType::BG_1;
            addr += 0x9600;
        } else {
            pixel_type = PixelType::BG_0;
        }
        if !self.check_window_bg(pixel_type) {
            return;
        }

        for i in 0..240 {
            self.update_cur_scanline_bg(
                i as usize,
                Ppu::process_palette_colour(
                    bus.read_byte_raw(addr + i, MemoryRegion::Vram),
                    false,
                    false,
                    bus,
                ),
                pixel_type,
            );
        }
    }

    // -------- tiled background processing
    fn process_tiled_bg(&mut self, pixel_type: PixelType, is_affine: bool, bus: &Bus) {
        let bg_num = pixel_type as usize;
        if !self.check_window_bg(pixel_type) {
            return;
        }
        let bg_cnt = bus.read_halfword_raw(0x8 + 2 * bg_num, MemoryRegion::IO);
        if self.cur_priority != bg_cnt as u8 & 0b11 || (self.disp_cnt >> (8 + bg_num)) & 1 == 0 {
            return;
        }
        let (w, h) = self.get_tiled_bg_dimensions(bg_cnt >> 14, is_affine);
        // if 0: 4bpp, if 1: 8bpp
        let density = is_affine || (bg_cnt >> 7) & 1 > 0;
        let wrapping = !is_affine || (bg_cnt >> 13) & 1 > 0;
        let base_screenblock_addr = ((bg_cnt as usize >> 8) & 0b11111) * 2048;
        let base_charblock_addr = ((bg_cnt as usize >> 2) & 0b11) * 0x4000;

        let x = 0 - bus.read_halfword_raw(0x10 + 4 * bg_num, MemoryRegion::IO);
        let y = 0 - bus.read_halfword_raw(0x12 + 4 * bg_num, MemoryRegion::IO);

        let i_rel = self.cur_line as u16 - y;

        let base_p_addr = 0x20 + 0x10 * (bg_num - 2);
        let pa = bus.read_halfword_raw(base_p_addr, MemoryRegion::IO) as i16 as i32;
        let pb = bus.read_halfword_raw(base_p_addr + 2, MemoryRegion::IO) as i16 as i32;
        let pc = bus.read_halfword_raw(base_p_addr + 4, MemoryRegion::IO) as i16 as i32;
        let pd = bus.read_halfword_raw(base_p_addr + 6, MemoryRegion::IO) as i16 as i32;

        let dx = bus.read_word_raw(0x28 + 0x10 * (bg_num - 2), MemoryRegion::IO) as i32;
        let dy = bus.read_word_raw(0x2c + 0x10 * (bg_num - 2), MemoryRegion::IO) as i32;

        for j in 0..240 {
            let j_rel = j - x;

            let mut ox = j_rel;
            let mut oy = i_rel;

            let (mut px, mut py, tile_addr);
            let mut pal_bank = 0; // NOTE: pal_bank is unused for affine backgrounds

            if is_affine {
                let cy = self.cur_line as i32;
                let cx = j as i32;

                ox = ((dx + pa * cx + pb * cy) >> 8) as u16;
                oy = ((dy + pc * cx + pd * cy) >> 8) as u16;

                if !wrapping && (ox >= w || oy >= h) {
                    // no wrapping, so pixel is out of bounds. do nothing
                    continue;
                } else {
                    ox %= w;
                    oy %= h;
                }

                let offset_screen_entry = (oy >> 3) * (w >> 3) + (ox >> 3);
                let screen_entry = bus.read_byte_raw(
                    base_screenblock_addr + offset_screen_entry as usize,
                    MemoryRegion::Vram,
                );
                px = ox & 0b111;
                py = oy & 0b111;

                tile_addr = base_charblock_addr + ((screen_entry as usize) << 6);
            } else {
                // get pixel data. assumes ox and oy are relative to the background.

                ox %= w;
                oy %= h;

                let mut cur_screenblock_addr = base_screenblock_addr;
                let mut ox_rel = ox;
                let mut oy_rel = oy;
                if !is_affine {
                    cur_screenblock_addr +=
                        ((oy as usize / 256) * w as usize / 256 + ox as usize / 256) * 2048;
                    // relative to current screenblock
                    ox_rel = ox % 256;
                    oy_rel = oy % 256;
                }

                //let offset_screen_entry = (oy_rel as usize >> 3) * 32 + (ox_rel as usize >> 3) * 64 + ((oy_rel as usize & 0b111) * 8 + (ox_rel as usize & 0b111));
                let offset_screen_entry = if !is_affine {
                    (oy_rel >> 3) * 32 + (ox_rel >> 3)
                } else {
                    (oy_rel >> 3) * (w >> 3) + (ox_rel >> 3)
                };
                let screen_entry = bus.read_halfword_raw(
                    cur_screenblock_addr + ((offset_screen_entry as usize) << 1),
                    MemoryRegion::Vram,
                );

                // relative to current tile
                px = ox_rel & 0b111;
                py = oy_rel & 0b111;

                if (screen_entry >> 10) & 1 > 0 {
                    px = 8 - px - 1;
                }
                if (screen_entry >> 11) & 1 > 0 {
                    py = 8 - py - 1;
                }

                pal_bank = ((screen_entry >> 12) << 4) as u8;

                tile_addr = base_charblock_addr
                    + (screen_entry as usize & 0b1111111111) * if density { 64 } else { 32 };
            }
            let offset_pixels = (py << 3) as usize + px as usize;

            let pal = if !density {
                let cur_addr = tile_addr + (offset_pixels >> 1);
                if offset_pixels & 1 > 0 {
                    (bus.read_byte_raw(cur_addr, MemoryRegion::Vram) >> 4) + pal_bank
                } else {
                    (bus.read_byte_raw(cur_addr, MemoryRegion::Vram) & 0b1111) + pal_bank
                }
            } else {
                let cur_addr = tile_addr + offset_pixels;
                bus.read_byte_raw(cur_addr, MemoryRegion::Vram)
            };

            //if self.cur_line == 10 && bg_num == 0 {
            //    info!("pal addr: {:#x}, screen_entry: {:#018b}, pixel colour: {:#018b}", pal, screen_entry, bus.read_halfword_raw(0x05000000 + pal as usize * 2));
            //}

            let pixel = Ppu::process_palette_colour(pal, !density, false, bus);
            self.update_cur_scanline_bg(j as usize, pixel, pixel_type);
        }
    }

    // returns width, height in pixels
    fn get_tiled_bg_dimensions(&self, sz_flag: u16, is_affine: bool) -> (u16, u16) {
        match (sz_flag, is_affine) {
            (0b00, false) => (256, 256),
            (0b01, false) => (512, 256),
            (0b10, false) => (256, 512),
            (0b11, false) => (512, 512),
            (0b00, true) => (128, 128),
            (0b01, true) => (256, 256),
            (0b10, true) => (512, 512),
            (0b11, true) => (1024, 1024),
            _ => {
                warn!(
                    "invalid sz_flag for tiled bg dimensions: {}, {}",
                    sz_flag, is_affine
                );
                (256, 256)
            }
        }
    }

    // -------- sprite processing

    // process_win_obj: if set true, no sprites are drawn. instead, updates windows.
    fn process_sprites(&mut self, process_win_obj: bool, bus: &Bus) {
        if !self.check_window_sprite(process_win_obj) || (self.disp_cnt >> 12) & 1 == 0 {
            return;
        }

        let map_mode = (self.disp_cnt >> 6) & 1 > 0; // 0 means 2D mapping. 1 means 1D mapping.
                                                     //let base_oam_addr = 0x7000000;

        for k in (0..128).rev() {
            // process sprite attributes
            let attr0 = bus.read_halfword_raw(k * 8, MemoryRegion::Oam);
            let obj_mode = (attr0 >> 8) & 0b11;
            if obj_mode == 0b10 {
                // no rendering
                continue;
            }
            let attr2 = bus.read_halfword_raw(k * 8 + 4, MemoryRegion::Oam);
            let cur_p = ((attr2 >> 10) & 0b11) as u8;
            if !process_win_obj && cur_p != self.cur_priority {
                continue;
            }

            let gfx = (attr0 >> 10) & 0b11;
            if process_win_obj && (gfx != 0b10) {
                continue;
            }

            let density = (attr0 >> 13) & 1 > 0; // 0 means 4 bits per pixel, 1 means 8 bits per pixel
            let pal_bank = ((attr2 >> 12) << 4) as u8;

            let attr1 = bus.read_halfword_raw(k * 8 + 2, MemoryRegion::Oam);
            let base_tile_index = attr2 & 0b1111111111;
            if self.disp_cnt & 0b111 >= 3 && base_tile_index < 512 {
                continue; // ignore lower charblock on bitmap modes
            }
            //let addr = base_tile_index as usize * 32 + 0x6010000;

            let y = attr0 & 0b11111111;
            let x = attr1 & 0b111111111;

            let affine = (attr0 >> 8) & 1 > 0;
            let affine_is_double = (attr0 >> 9) > 0;
            let affine_obj_addr = ((attr1 >> 9) & 0b11111) as usize * 32;
            let pa = bus.read_halfword_raw(affine_obj_addr + 6, MemoryRegion::Oam);
            let pb = bus.read_halfword_raw(affine_obj_addr + 14, MemoryRegion::Oam);
            let pc = bus.read_halfword_raw(affine_obj_addr + 22, MemoryRegion::Oam);
            let pd = bus.read_halfword_raw(affine_obj_addr + 30, MemoryRegion::Oam);

            let y_flip = (attr1 >> 13) & 1 > 0;
            let x_flip = (attr1 >> 12) & 1 > 0;

            // width, height in pixels
            let (w, h) = self.get_sprite_dimensions((attr0 >> 14) as u8, (attr1 >> 14) as u8);
            let (mut affine_w, mut affine_h) = (w, h);
            if affine && affine_is_double {
                affine_w *= 2;
                affine_h *= 2;
            }
            // NOTE: these pixels are replaced directly (not using Pixel::overwrite())

            let i = self.cur_line - y as u8;
            //if affine && affine_is_double {
            //    i += (h >> 1) as u8;
            //}
            let i = i as u16;
            if i >= affine_h {
                continue;
            }
            for j in 0..affine_w {
                let (ox, oy, read_pixel);
                if !affine {
                    oy = if y_flip { h - i - 1 } else { i };
                    ox = if x_flip { w - j - 1 } else { j };
                    read_pixel = true;
                } else {
                    //let j = j - x;
                    let cx = (Wrapping(j) - Wrapping(affine_w >> 1)).0;
                    let cy = (Wrapping(i) - Wrapping(affine_h >> 1)).0;
                    ox = ((pa * cx + pb * cy) as i16 >> 8) as u16 + (w as u16 >> 1);
                    oy = ((pc * cx + pd * cy) as i16 >> 8) as u16 + (h as u16 >> 1);

                    read_pixel = ox < w && oy < h;
                };
                if read_pixel {
                    // byte offset for each pixel
                    let offset_pixels = (oy as usize >> 3) * (w as usize >> 3) * 64
                        + (ox as usize >> 3) * 64
                        + ((oy as usize & 0b111) * 8 + (ox as usize & 0b111));
                    let pal = if !density {
                        // 4 bits per pixel
                        let mut cur_addr = base_tile_index as usize * 32 + (offset_pixels >> 1);
                        if !map_mode {
                            cur_addr += ((oy as usize >> 3) * (128 - (w as usize >> 1))) << 3;
                        }
                        let cur_addr = 0x10000 + (cur_addr % 32768);
                        if offset_pixels & 1 > 0 {
                            (bus.read_byte_raw(cur_addr, MemoryRegion::Vram) >> 4) + pal_bank
                        } else {
                            (bus.read_byte_raw(cur_addr, MemoryRegion::Vram) & 0b1111) + pal_bank
                        }
                    } else {
                        // 8 bits per pixel
                        let mut cur_addr = base_tile_index as usize * 32 + offset_pixels;
                        if !map_mode {
                            cur_addr += ((oy as usize >> 3) * (128 - w as usize)) << 3;
                        }
                        let cur_addr = 0x10000 + (cur_addr % 32768);
                        bus.read_byte_raw(cur_addr, MemoryRegion::Vram)
                    };
                    let pixel = Ppu::process_palette_colour(pal, !density, true, bus);

                    let mut tx = j as usize + x as usize;
                    //if affine && affine_is_double{
                    //    tx -= w as usize >> 1;
                    //}
                    tx &= 0b111111111;
                    if tx < 240 {
                        if gfx == 0b10 && !process_win_obj {
                            continue;
                        }
                        if !process_win_obj {
                            self.update_cur_scanline_sprite(tx, pixel, gfx == 1);
                        } else if pixel.is_some() {
                            self.set_window_scanline(WindowType::W_obj, tx);
                        }
                    }
                }
            }
        }
    }

    // returns width, height in terms of pixels
    fn get_sprite_dimensions(&self, shape: u8, size: u8) -> (u16, u16) {
        match (shape, size) {
            (0b00, 0b00) => (8, 8),
            (0b00, 0b01) => (16, 16),
            (0b00, 0b10) => (32, 32),
            (0b00, 0b11) => (64, 64),
            (0b01, 0b00) => (16, 8),
            (0b01, 0b01) => (32, 8),
            (0b01, 0b10) => (32, 16),
            (0b01, 0b11) => (64, 32),
            (0b10, 0b00) => (8, 16),
            (0b10, 0b01) => (8, 32),
            (0b10, 0b10) => (16, 32),
            (0b10, 0b11) => (32, 64),
            _ => {
                warn!("invalid sprite shape and/or size");
                (8, 8)
            }
        }
    }

    // ------- windows

    fn init_window_scanline(&mut self, bus: &Bus) {
        self.is_windowing_active = (self.disp_cnt >> 13) > 0;
        self.active_windows[0] = false;
        self.active_windows[1] = false;
        self.active_windows[2] = false;
        self.active_windows[3] = false;
        if !self.is_windowing_active {
            return;
        }

        self.window_scanlines[3].iter_mut().for_each(|x| *x = true);
        self.window_flags[WindowType::W_out as usize] = bus.read_byte_raw(0x4a, MemoryRegion::IO);
        self.active_windows[3] = true;
        //info!("W_out: {:#010b}", self.window_flags[WindowType::W_out as usize]);

        if self.disp_cnt >> (13 + WindowType::W_0 as u16) & 1 > 0 {
            self.window_scanlines[0].iter_mut().for_each(|x| *x = false);
            // set w0
            let (l, r) = (
                bus.read_byte_raw(0x45, MemoryRegion::IO),
                bus.read_byte_raw(0x44, MemoryRegion::IO),
            );
            if (l <= r && self.cur_line >= l && self.cur_line < r)
                || (l > r && (self.cur_line >= l || self.cur_line < r))
            {
                let (l, mut r) = (
                    bus.read_byte_raw(0x41, MemoryRegion::IO) as u16,
                    bus.read_byte_raw(0x40, MemoryRegion::IO) as u16,
                );
                if l > r {
                    r += 1 << 8;
                }
                for i in l..r {
                    self.set_window_scanline(WindowType::W_0, i as u8 as usize);
                }
            }
            self.window_flags[WindowType::W_0 as usize] = bus.read_byte_raw(0x48, MemoryRegion::IO);
            //info!("W_0: {:#010b}", self.window_flags[WindowType::W_0 as usize]);
        }
        if self.disp_cnt >> (13 + WindowType::W_1 as u16) & 1 > 0 {
            self.window_scanlines[1].iter_mut().for_each(|x| *x = false);
            // set w1
            let (l, r) = (
                bus.read_byte_raw(0x47, MemoryRegion::IO),
                bus.read_byte_raw(0x46, MemoryRegion::IO),
            );
            if (l <= r && self.cur_line >= l && self.cur_line < r)
                || (l > r && (self.cur_line >= l || self.cur_line < r))
            {
                let (l, mut r) = (
                    bus.read_byte_raw(0x43, MemoryRegion::IO) as u16,
                    bus.read_byte_raw(0x42, MemoryRegion::IO) as u16,
                );
                if l > r {
                    r += 1 << 8;
                }
                for i in l..r {
                    self.set_window_scanline(WindowType::W_1, i as u8 as usize);
                }
            }
            self.window_flags[WindowType::W_1 as usize] = bus.read_byte_raw(0x49, MemoryRegion::IO);
            //info!("W_1: {:#010b}", self.window_flags[WindowType::W_1 as usize]);
        }

        if self.disp_cnt >> (13 + WindowType::W_obj as u16) & 1 > 0 {
            self.window_scanlines[2].iter_mut().for_each(|x| *x = false);
            self.process_sprites(true, bus);
            self.window_flags[WindowType::W_obj as usize] =
                bus.read_byte_raw(0x4b, MemoryRegion::IO);
            //info!("W_obj: {:#010b}", self.window_flags[WindowType::W_obj as usize]);
        }
    }

    // note: do not use for sprites
    fn check_window_bg(&self, pixel_type: PixelType) -> bool {
        self.cur_window == WindowType::W_full
            || (self.window_flags[self.cur_window as usize] >> (pixel_type as u8)) & 1 > 0
    }

    fn check_window_sprite(&self, process_win_obj: bool) -> bool {
        process_win_obj
            || self.cur_window == WindowType::W_full
            || (self.window_flags[self.cur_window as usize] >> 4) & 1 > 0
    }

    //fn should_update_pixel(&mut self, index: usize) -> bool {
    //    self.cur_window == WindowType::W_full || self.get_window_scanline(self.cur_window, index)
    //}

    fn update_cur_scanline_bg(
        &mut self,
        index: usize,
        pixel: Option<Pixel>,
        pixel_type: PixelType,
    ) {
        if self.cur_window == WindowType::W_full || self.get_window_scanline(self.cur_window, index)
        {
            if let Some(pixel) = pixel {
                self.cur_scanline_back[index] = self.cur_scanline_front[index];
                self.cur_scanline_front[index] = (pixel, pixel_type, self.cur_window);
                //self.cur_scanline[index].overwrite(&pixel);
            }
        }
    }

    fn update_cur_scanline_sprite(&mut self, index: usize, pixel: Option<Pixel>, is_blend: bool) {
        if self.cur_window == WindowType::W_full || self.get_window_scanline(self.cur_window, index)
        {
            if let Some(pixel) = pixel {
                if self.cur_scanline_front[index].1 != PixelType::Sprite
                    && self.cur_scanline_front[index].1 != PixelType::Sprite_blend
                {
                    self.cur_scanline_back[index] = self.cur_scanline_front[index];
                }
                self.cur_scanline_front[index] = (
                    pixel,
                    if is_blend {
                        PixelType::Sprite_blend
                    } else {
                        PixelType::Sprite
                    },
                    self.cur_window,
                );
            }
        }
    }

    // NOTE: do not set WindowType::W_out directly
    #[inline(always)]
    fn set_window_scanline(&mut self, window_type: WindowType, ind: usize) {
        if ind < 240 && self.window_scanlines[WindowType::W_out as usize][ind] {
            self.window_scanlines[window_type as usize][ind] = true;
            self.window_scanlines[WindowType::W_out as usize][ind] = false;
            self.active_windows[window_type as usize] = true;
        }
    }

    #[inline(always)]
    fn get_window_scanline(&mut self, window_type: WindowType, ind: usize) -> bool {
        self.window_scanlines[window_type as usize][ind]
    }

    // ------- helper functions

    fn process_15bit_colour(halfword: u16) -> Pixel {
        Pixel::new(
            (halfword & 0b11111) as u8,
            ((halfword >> 5) & 0b11111) as u8,
            ((halfword >> 10) & 0b11111) as u8,
        )
    }

    fn process_palette_colour(
        palette_index: u8,
        is_4bpp: bool,
        is_sprite: bool,
        bus: &Bus,
    ) -> Option<Pixel> {
        if palette_index == 0 || (is_4bpp && (palette_index & 0b1111) == 0) {
            return None;
        }
        let mut addr = palette_index as u32 * 2;
        if is_sprite {
            addr += 0x200;
        }
        let colour = bus.read_halfword_raw(addr as usize, MemoryRegion::Palette);
        Some(Ppu::process_15bit_colour(colour))
    }

    // -------- old code, kept for reference
    /*
    fn process_sprites(&mut self, bus: &Bus) {
        if (self.disp_cnt >> 12) & 1 == 0{
            return;
        }

        let map_mode = (self.disp_cnt >> 6) & 1 > 0; // 0 means 2D mapping. 1 means 1D mapping.
        let base_oam_addr = 0x7000000;

        for k in (0..128).rev() {
            // process sprite attributes
            let attr0 = bus.read_halfword_raw(base_oam_addr + k * 8);
            let obj_mode = (attr0 >> 8) & 0b11;
            if obj_mode == 0b10 {
                // no rendering
                continue;
            }
            let attr2 = bus.read_halfword_raw(base_oam_addr + k * 8 + 4);
            let cur_p = ((attr2 >> 10) & 0b11) as u8;
            if cur_p != self.cur_priority{
                continue;
            }

            let density = (attr0 >> 13) & 1 > 0; // 0 means 4 bits per pixel, 1 means 8 bits per pixel
            let pal_bank = ((attr2 >> 12) << 4) as u8;

            /*let tile_size = if density {
                64
            }
            else{
                32
            };*/

            let attr1 = bus.read_halfword_raw(base_oam_addr + k * 8 + 2);
            let base_tile_index = attr2 & 0b1111111111;
            if self.disp_cnt & 0b111 >= 3 && base_tile_index < 512 {
                continue; // ignore lower charblock on bitmap modes
            }
            //let addr = base_tile_index as usize * 32 + 0x6010000;

            let y = attr0 & 0b11111111;
            let x = attr1 & 0b111111111;

            let affine = (attr0 >> 8) & 1 > 0;

            let y_flip = (attr1 >> 12) & 1 > 0;
            let x_flip = (attr1 >> 13) & 1 > 0;

            // width, height in pixels
            let (w,h) = self.get_sprite_dimensions((attr0 >> 14) as u8, (attr1 >> 14) as u8);
            //let row_size = tile_size as usize * w as usize / 8;

            // todo: consider 2d mapping. the below needs to be changed
            //let tile_data = bus.bulk_read_byte(addr, tile_size as usize * w as usize * h as usize / 64);

            //if self.cur_line == 1 {
            //    info!("base tile index: {:#x}", base_tile_index);
            //}

            // NOTE: these pixels are replaced directly (not using Pixel::overwrite())


            for i in 0..h as usize{

                //let row = bus.bulk_read_word(addr + row_size * i, row_size);
                for j in 0..w as usize{
                    let offset_pixels = (i >> 3) * (w as usize >> 3) * 64 + (j >> 3) * 64 + ((i & 0b111) * 8 + (j & 0b111));

                    let pal =
                    // 4 bits per pixel
                    if !density {
                        let cur_addr = 0x6010000 + (base_tile_index as usize * 32 + (offset_pixels >> 1)) % 32768;
                        if offset_pixels & 1 > 0{
                            (bus.read_byte_raw(cur_addr) >> 4) + pal_bank
                        }
                        else{
                            (bus.read_byte_raw(cur_addr) & 0b1111) + pal_bank
                        }
                    }
                    // 8 bits per pixel
                    else{
                        let cur_addr = 0x6010000 + (base_tile_index as usize * 32 + offset_pixels) % 32768;
                        bus.read_byte_raw(cur_addr)
                    };
                    let pixel = self.process_palette_colour(pal, !density, true, bus);

                    //if self.cur_line == 0 && i == 5 && j == 3{
                    //    //info!("r {}, g: {}, b: {}", pixel.to_float().0, pixel.to_float().1, pixel.to_float().2);
                    //    info!("addr: {:#x}, addr_row: {:#x}", addr, addr + row_size * i);
                    //}

                    let (i_final, j_final) =
                    if !affine{
                        let i_trans = if y_flip {
                            w as usize-i-1
                            //w as u16-i as u16-1
                        }
                        else{
                            i
                            //i as u16
                        };

                        let j_trans = if x_flip {
                            h as usize-j-1
                            //h as u16 - j as u16 - 1
                        }
                        else{
                            j
                            //j as u16
                        };

                        //if i_trans as u8 + y as u8 == self.cur_line && (j_trans + x as usize) & 0b11111111 < 240{
                        //    self.cur_scanline[(j_trans + x as usize) & 0b11111111].overwrite(&pixel);
                        //}
                        (i_trans, j_trans)

                        //((i_trans + y) as u8 as u16, (j_trans + x as u16) & 0b111111111)
                    }
                    else{
                        (i,j)
                    };
                    self.sprite_buff[i_final][j_final] = pixel;
                    /*
                    else{
                        let p0x = x + (w as u16 >> 1);
                        let p0y = y + (h as u16 >> 1);
                        let cx = x + j as u16 - p0x;
                        let cy = y + i as u16 - p0y;
                        (((pc*cx + pd*cy) >> 8) + p0y, ((pa*cx + pb*cy) >> 8) + p0x)
                    };
                    let is_in_rect = if !affine || !affine_is_double {
                        i_final >= y as u16 && i_final < y as u16 + h as u16 &&
                        j_final >= x as u16 && j_final < x as u16 + w as u16
                    }
                    else{
                        i_final >= y - (h as u16 >> 1) && i_final < y + (h as u16 >> 1) * 3 &&
                        j_final >= x - (w as u16 >> 1) && j_final < x + (w as u16 >> 1) * 3
                    };
                    if is_in_rect && i_final as u8 == self.cur_line && j_final < 240{
                        self.cur_scanline[j_final as usize].overwrite(pixel);
                    }*/
                }
            }

            //let final_sprite_buff =

            if !affine{
                self.draw_sprite_buff(false, false, x as usize, y as usize, w as usize, h as usize);
            }
            else{
                let affine_is_double = (attr0 >> 9) > 0;
                let affine_obj_addr = ((attr1 >> 9) & 0b11111) as usize * 32 + base_oam_addr;
                let pa = bus.read_halfword_raw(affine_obj_addr + 6);
                let pb = bus.read_halfword_raw(affine_obj_addr + 14);
                let pc = bus.read_halfword_raw(affine_obj_addr + 22);
                let pd = bus.read_halfword_raw(affine_obj_addr + 30);

                let mut affine_w = w as u16;
                let mut affine_h = h as u16;
                if affine_is_double {
                    affine_w *= 2;
                    affine_h *= 2;
                }

                for i in 0..affine_h {
                    for j in 0..affine_w {
                        let cx = (Wrapping(j) - Wrapping(affine_w >> 1)).0;
                        let cy = (Wrapping(i) - Wrapping(affine_h >> 1)).0;

                        let ox = ((pa*cx + pb*cy) as i16 >> 8) as u16 + (w as u16 >> 1);
                        let oy = ((pc*cx + pd*cy) as i16 >> 8) as u16  + (h as u16 >> 1);

                        //if i==0 && j==0 {
                        //    info!("cx: {}, cy: {}, ox: {}, oy: {}", cx, cy, ox, oy);
                        //}

                        if ox < w as u16 && oy < h as u16 {
                            self.affine_sprite_buff[i as usize][j as usize] = self.sprite_buff[oy as usize][ox as usize];
                        }
                        else{
                            self.affine_sprite_buff[i as usize][j as usize] = Pixel::Transparent;
                        }
                    }
                }

                self.draw_sprite_buff(true, affine_is_double, x as usize, y as usize, w as usize, h as usize);
            }
        }

    }

    fn draw_sprite_buff(&mut self, is_affine: bool, affine_is_double: bool, x: usize, y: usize, w: usize, h: usize) {
        let mut i = self.cur_line as usize - y as usize;
        if affine_is_double{
            i += h >> 1;
        }
        if i < h {
            for j in 0..w{
                let mut j_trans = j + x;
                if affine_is_double{
                    j_trans -= w >> 1;
                }
                if j_trans & 0b111111111 < 240{
                    let pixel = if is_affine {
                        self.affine_sprite_buff[i][j]
                    }
                    else{
                        self.sprite_buff[i][j]
                    };
                    self.cur_scanline[j_trans & 0b111111111].overwrite(&pixel);
                }
            }
        }
    }
    */
}
