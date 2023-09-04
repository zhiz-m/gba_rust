use std::ops::{Deref, DerefMut};

use crate::{
    bus::{Bus, MemoryRegion},
    config,
};
use log::{info, warn};
use rubato::{FftFixedInOut, Resampler};
use serde::{Serialize, Deserialize};

// StereoTuple.0 is right, StereoTuple.1 is left
#[derive(Serialize, Deserialize)]
struct StereoTuple(Option<i16>, Option<i16>);
impl StereoTuple {
    pub fn new() -> StereoTuple {
        StereoTuple(None, None)
    }
    pub fn add(&mut self, channel: usize, val: i16) {
        match channel {
            0 => {
                self.0 = match self.0 {
                    None => Some(val),
                    Some(cur) => Some(cur + val),
                }
            }
            1 => {
                self.1 = match self.1 {
                    None => Some(val),
                    Some(cur) => Some(cur + val),
                }
            }
            _ => unreachable!(),
        }
    }
    pub fn add_bias(&mut self, channel: usize, val: i16) {
        match channel {
            0 => {
                self.0 = self.0.map(|cur| cur + val);
            }
            1 => {
                self.1 = self.1.map(|cur| cur + val);
            }
            _ => unreachable!(),
        }
    }
    /*pub fn multiply(&mut self, channel: usize, val: i16) {
        match channel {
            0 => {
                self.0 = self.0.map(|cur| cur * val);
            }
            1 => {
                self.1 = self.1.map(|cur| cur * val);
            }
            _ => unreachable!(),
        }
    }*/
    pub fn clip(&mut self) {
        self.0 = self
            .0
            .map(|val| std::cmp::max(0, std::cmp::min(0x3ff, val)));
        self.1 = self
            .1
            .map(|val| std::cmp::max(0, std::cmp::min(0x3ff, val)));
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FifoQueue {
    mem: Vec<i8>,
    write_ind: usize,
    read_ind: usize,
    capacity: usize,
}

impl FifoQueue {
    pub fn new() -> FifoQueue {
        FifoQueue {
            mem: vec![0; 32],
            write_ind: 0,
            read_ind: 0,
            capacity: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.capacity
    }
    pub fn pop_front(&mut self) -> Option<i8> {
        if self.capacity == 0 {
            return None;
        }
        let res = self.mem[self.read_ind];
        self.read_ind = (self.read_ind + 1) & 31;
        self.capacity -= 1;
        Some(res)
    }
    pub fn push_back(&mut self, val: i8) {
        self.mem[self.write_ind] = val;
        self.write_ind = (self.write_ind + 1) & 31;
        self.capacity += 1;
    }
    pub fn clear(&mut self) {
        self.write_ind = 0;
        self.read_ind = 0;
        self.capacity = 0;
        self.mem.fill(0);
    }
}

pub struct SoundBufferIt<'a> {
    data: &'a [Vec<Vec<f32>>],
    index_outer: usize,
    index_inner: usize,
}

impl<'a> Iterator for SoundBufferIt<'a> {
    type Item = (f32, f32);
    fn next(&mut self) -> Option<(f32, f32)> {
        if self.index_outer == self.data.len() {
            return None;
        }
        let res = (
            self.data[self.index_outer][0][self.index_inner],
            self.data[self.index_outer][1][self.index_inner],
        );
        self.index_inner += 1;
        if self.index_inner == self.data[self.index_outer][0].len() {
            self.index_outer += 1;
            self.index_inner = 0;
        }
        Some(res)
    }
}

impl<'a> SoundBufferIt<'a>{
    pub fn len(&self) -> usize{
        self.data.iter().map(|x|x.len()).sum()
    }
}

pub struct SamplerWrapper(usize, FftFixedInOut<f32>);

impl SamplerWrapper{
    fn new(sample_rate_output: usize) -> SamplerWrapper{
        let sampler = FftFixedInOut::new(
            config::AUDIO_SAMPLE_RATE as usize,
            sample_rate_output,
            config::AUDIO_SAMPLE_CHUNKS,
            2,
        )
        .unwrap();

        SamplerWrapper(sample_rate_output, sampler)
    }
}

impl Deref for SamplerWrapper{
    type Target = FftFixedInOut<f32>;

    fn deref(&self) -> &Self::Target {
        &self.1
    }

}

impl DerefMut for SamplerWrapper{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.1
    }
}

mod sampler_as_u64 {
    use serde::{Serializer, Deserializer, Deserialize};

    use super::SamplerWrapper;

    pub fn serialize<S>(value: &SamplerWrapper, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize the string as is
        serializer.serialize_u64(value.0 as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SamplerWrapper, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize the string and parse it into an integer
        let sample_rate_output: u64 = u64::deserialize(deserializer)?;
        Ok(SamplerWrapper::new(sample_rate_output as usize))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Apu {
    //  ------- square sound channels
    square_length: [u32; 2],
    square_rate: [u32; 2],
    square_envelope: [u32; 2],

    // counts number of clock cycles
    square_sweep_cnt: [u32; 2],
    square_envelope_cnt: [u32; 2],

    pub square_disable: [bool; 2],

    // -------- wave sound channel
    wave_length: u32,
    wave_rate: u32,
    pub wave_sweep_cnt: u32,
    pub wave_bank: Vec<Vec<u8>>,

    // -------- direct sound (DMA) channels
    pub direct_sound_fifo: Vec<FifoQueue>,
    //pub direct_sound_fifo: Vec<VecDeque<i8>>,
    pub direct_sound_fifo_cur: [i8; 2],
    pub direct_sound_timer: [Option<usize>; 2],

    sound_in_buff: Vec<Vec<f32>>,
    sound_out_buff: Vec<Vec<Vec<f32>>>,
    sound_out_buff_index: usize,
    #[serde(with = "sampler_as_u64")]
    sampler: SamplerWrapper,
    sample_rate_output: usize,

    pub extern_audio_enabled: bool,
}

impl Apu {
    pub fn new(sample_rate_output: usize) -> Apu {
        /*let params = InterpolationParameters{
            sinc_len: 256,
            f_cutoff: 0.95,
            oversampling_factor: 128,
            interpolation: InterpolationType::Cubic,
            window: rubato::WindowFunction::Hann,
        };
        let sampler = SincFixedIn::new(sample_rate_output as f64 / config::AUDIO_SAMPLE_RATE as f64, 1f64, params, 1024, 2).unwrap();
        */
        let sampler = FftFixedInOut::new(
            config::AUDIO_SAMPLE_RATE as usize,
            sample_rate_output,
            config::AUDIO_SAMPLE_CHUNKS,
            2,
        )
        .unwrap();
        let sound_out_buff_extern_size = 16 * 1024 * 1024 / config::AUDIO_SAMPLE_CHUNKS;

        info!(
            "sampler input required size: {}",
            sampler.input_frames_next()
        );
        Apu {
            square_length: [0; 2],
            square_rate: [0; 2],
            square_envelope: [0; 2],

            square_sweep_cnt: [0; 2],
            square_envelope_cnt: [0; 2],

            square_disable: [false; 2],

            wave_length: 0,
            wave_rate: 0,
            wave_sweep_cnt: 0,
            wave_bank: vec![vec![0; 16]; 2],

            direct_sound_fifo: vec![FifoQueue::new(); 2],
            //direct_sound_fifo: vec![VecDeque::<i8>::with_capacity(32); 2],
            direct_sound_fifo_cur: [0; 2],
            direct_sound_timer: [None; 2],

            sound_in_buff: sampler.input_buffer_allocate(),
            sound_out_buff: vec![sampler.output_buffer_allocate(); sound_out_buff_extern_size],
            sound_out_buff_index: 0,
            sampler: SamplerWrapper::new(sample_rate_output),
            sample_rate_output,

            extern_audio_enabled: true,
        }
    }

    #[inline(always)]
    pub fn get_audio_buffer(&mut self) -> Option<SoundBufferIt> {
        if self.extern_audio_enabled {
            Some(SoundBufferIt {
                data: &self.sound_out_buff[0..self.sound_out_buff_index],
                index_outer: 0,
                index_inner: 0,
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn clear_buffer(&mut self) {
        self.sound_out_buff_index = 0;
    }

    // called every config::AUDIO_SAMPLE_CLOCKS clocks
    #[inline(always)]
    pub fn clock(&mut self, bus: &mut Bus) {
        self.square_disable[0] = false;
        self.square_disable[1] = false;
        let mut cur_tuple = StereoTuple::new();
        let snd_stat = bus.read_byte_raw(0x84, MemoryRegion::IO);
        if (snd_stat >> 7) & 1 > 0 {
            // sound enabled
            let snd_dmg_cnt = bus.read_halfword_raw(0x80, MemoryRegion::IO);
            //info!("snd_dmg_cnt: {:#018b}", snd_dmg_cnt);
            //info!("bias: {:#018b}", bus.read_halfword_raw(0x4000088));
            let snd_ds_cnt = bus.read_halfword_raw(0x82, MemoryRegion::IO);

            let dmg_vol = [
                snd_dmg_cnt as i16 & 0b111,
                (snd_dmg_cnt >> 4) as i16 & 0b111,
            ];

            // square channels
            for i in 0..2 {
                let enable_right_left = [
                    (snd_dmg_cnt >> (8 + i)) & 1 > 0,
                    (snd_dmg_cnt >> (12 + i)) & 1 > 0,
                ];
                // sound is not enabled on any channel (left or right)
                if !enable_right_left[0] && !enable_right_left[1] {
                    continue;
                }
                let snd_cur_freq = bus.read_halfword(0x04000064 + 8 * i);

                if (snd_cur_freq >> 0xe) & 1 > 0 && self.square_length[i] == 0 {
                    continue;
                }
                // process sweep
                if i == 0 {
                    let snd_sweep = bus.read_byte(0x04000060);
                    let sweep_cnt_hit = ((snd_sweep as u32 >> 4) & 0b111) << 17;
                    let sweep_num = snd_sweep & 0b111;
                    if sweep_cnt_hit != 0
                        && sweep_num != 0
                        && self.square_sweep_cnt[i] >= sweep_cnt_hit
                    {
                        let rate_delta = self.square_rate[i] >> sweep_num;
                        if (snd_sweep >> 3) & 1 > 0 {
                            self.square_rate[i] -= rate_delta;
                        }
                        // would overflow, disable current channel
                        else if 2048 - self.square_rate[i] <= rate_delta {
                            self.square_disable[i] = true;
                            continue;
                        } else {
                            self.square_rate[i] += rate_delta;
                        }
                        self.square_sweep_cnt[i] = 0;
                    }
                }
                let snd_cur_cnt = bus.read_halfword(0x04000062 + i * 6);

                // process envelope changes
                let envelope_cnt_hit = ((snd_cur_cnt as u32 >> 8) & 0b111) << 18;
                let envelope_increase = (snd_cur_cnt >> 0xb) & 1 > 0;
                if envelope_cnt_hit != 0
                    && !((envelope_increase && self.square_envelope[i] == 0b1111)
                        || (!envelope_increase && self.square_envelope[i] == 0))
                {
                    if self.square_envelope_cnt[i] >= envelope_cnt_hit {
                        if envelope_increase {
                            self.square_envelope[i] += 1;
                        } else {
                            self.square_envelope[i] -= 1;
                        }
                        self.square_envelope_cnt[i] = 0;
                    }
                    self.square_envelope_cnt[i] += config::AUDIO_SAMPLE_CLOCKS;
                }

                // process duty cycle
                let period_clocks = (2048 - self.square_rate[i]) << 7;
                let active_clocks = match (snd_cur_cnt >> 6) & 0b11 {
                    0b00 => period_clocks >> 3,
                    0b01 => period_clocks >> 2,
                    0b10 => period_clocks >> 1,
                    0b11 => (period_clocks >> 2) * 3,
                    _ => unreachable!(),
                };

                // sound channels
                for j in 0..2 {
                    if !enable_right_left[j] {
                        continue;
                    }
                    let final_square_vol = match snd_ds_cnt & 0b11 {
                        0b00 => self.square_envelope[i] >> 2,
                        0b01 => self.square_envelope[i] >> 1,
                        0b10 => self.square_envelope[i],
                        0b11 => {
                            warn!("sound channel 1-4 has a volume of 0b11: forbidden");
                            self.square_envelope[i]
                        }
                        _ => unreachable!(),
                    } as i16;
                    if self.square_sweep_cnt[i] % period_clocks < active_clocks {
                        cur_tuple.add(j, final_square_vol * dmg_vol[j]);
                    } else {
                        cur_tuple.add(j, -final_square_vol * dmg_vol[j]);
                    }
                }

                self.square_sweep_cnt[i] += config::AUDIO_SAMPLE_CLOCKS;
                if self.square_length[i] > 0 {
                    self.square_length[i] -= config::AUDIO_SAMPLE_CLOCKS;
                }
            }

            // wave channel
            self.process_wave_channel(&mut cur_tuple, bus);

            // Direct Sound
            for i in 0..2 {
                let enable_right_left = [
                    (snd_ds_cnt >> (8 + 4 * i)) & 1 > 0,
                    (snd_ds_cnt >> (9 + 4 * i)) & 1 > 0,
                ];
                if !enable_right_left[0] && !enable_right_left[1] {
                    continue;
                }
                // sound right and left channels
                for (j, item) in enable_right_left.iter().enumerate() {
                    if !*item {
                        continue;
                    }
                    let final_sample = match (snd_ds_cnt >> (2 + j)) & 1 {
                        0 => self.direct_sound_fifo_cur[i] >> 1,
                        1 => self.direct_sound_fifo_cur[i],
                        _ => unreachable!(),
                    };
                    /*if final_sample as i16 != 0 {
                        //info!("playing from direct sound: {:#x}, ds_cnt: {:#018b}, channel: {}, snd_bias: {:#018b}", final_sample, snd_ds_cnt, i, bus.read_halfword(0x04000088));
                    } else {
                        //info!("direct sound zero");
                    }*/
                    cur_tuple.add(j, (final_sample as i16) * 4);
                }
            }

            // process volume
            //cur_tuple.multiply(0, snd_dmg_cnt as i16 & 0b111);
            //cur_tuple.multiply(1, (snd_dmg_cnt >> 4) as i16 & 0b111);

            // process bias
            let snd_bias = bus.read_word_raw(0x88, MemoryRegion::IO);
            let bias = snd_bias & 0b1111111111;
            cur_tuple.add_bias(0, bias as i16);
            cur_tuple.add_bias(1, bias as i16);

            // clip values into range [0, 0x3ff]
            cur_tuple.clip();
        }
        //else{
        //    info!("sound is off");
        //}

        // output channel 0 is left not right
        self.sound_in_buff[1].push(match cur_tuple.0 {
            None => 0f32,
            Some(val) => (val as f32 - 512.) / 512.,
        });
        self.sound_in_buff[0].push(match cur_tuple.1 {
            None => 0f32,
            Some(val) => (val as f32 - 512.) / 512.,
        });

        if self.sound_in_buff[0].len() == self.sampler.input_frames_next() {
            if self.extern_audio_enabled {
                self.sampler
                    .process_into_buffer(
                        &self.sound_in_buff,
                        &mut self.sound_out_buff[self.sound_out_buff_index],
                        None,
                    )
                    .unwrap();
                self.sound_out_buff_index += 1;
            }
            self.sound_in_buff[0].clear();
            self.sound_in_buff[1].clear();
            //self.sound_out_buff[0].clear();
            //self.sound_out_buff[1].clear();
        }
    }

    #[inline(always)]
    fn process_wave_channel(&mut self, cur_tuple: &mut StereoTuple, bus: &mut Bus) {
        let snd_cur_cnt_l = bus.read_byte_raw(0x70, MemoryRegion::IO);
        if snd_cur_cnt_l >> 7 == 0 {
            //info!("wave channel disabled");
            return;
        }

        let snd_dmg_cnt = bus.read_halfword_raw(0x80, MemoryRegion::IO);
        let dmg_vol = [
            snd_dmg_cnt as i16 & 0b111,
            (snd_dmg_cnt >> 4) as i16 & 0b111,
        ];
        //info!("snd_dmg_cnt: {:#018b}", snd_dmg_cnt);
        let snd_ds_cnt = bus.read_halfword_raw(0x82, MemoryRegion::IO);
        let enable_right_left = [(snd_dmg_cnt >> 10) & 1 > 0, (snd_dmg_cnt >> 14) & 1 > 0];
        // sound is not enabled on any channel (left or right)
        if !enable_right_left[0] && !enable_right_left[1] {
            return;
        }
        let snd_cur_freq = bus.read_halfword(0x04000074);

        if (snd_cur_freq >> 0xe) & 1 > 0 && self.wave_length == 0 {
            return;
        }
        let snd_cur_cnt_h = bus.read_halfword(0x04000072);
        let bank = (snd_cur_cnt_l >> 5) & (snd_cur_cnt_l >> 6) & 1;

        let period_clocks = (2048 - self.wave_rate) << 3;
        let ind = self.wave_sweep_cnt / period_clocks;

        let mut final_wave_vol = if true {
            self.wave_bank[bank as usize][((ind & 31) >> 1) as usize] as i16
        } else {
            //info!("wave bank is at its end, {:#010b}", snd_cur_cnt_l);
            0
        };

        if ind & 1 > 0 {
            final_wave_vol &= 0b1111;
        } else {
            final_wave_vol >>= 4;
        }

        // make signed. MAYBE UNDO: do not do this if audio is unsigned.
        /*if (final_wave_vol >> 3) & 1 > 0 {
            final_wave_vol |= !0b1111;
        }*/

        final_wave_vol = match snd_ds_cnt & 0b11 {
            0b00 => final_wave_vol >> 2,
            0b01 => final_wave_vol >> 1,
            0b10 => final_wave_vol,
            0b11 => {
                warn!("sound channel 1-4 has a volume of 0b11: forbidden");
                final_wave_vol
            }
            _ => unreachable!(),
        };

        final_wave_vol = match snd_cur_cnt_h >> 15 {
            0 => match (snd_cur_cnt_h >> 13) & 0b11 {
                0b00 => 0,
                0b01 => final_wave_vol,
                0b10 => final_wave_vol >> 1,
                0b11 => final_wave_vol >> 2,
                _ => unreachable!(),
            },
            1 => (final_wave_vol >> 2) * 3,
            _ => unreachable!(),
        };

        // sound channels
        for j in 0..2 {
            if !enable_right_left[j] {
                continue;
            }
            if final_wave_vol != 0 {
                //info!("playing wave sample: {:#018b}", final_wave_vol * dmg_vol[j]);
            }
            cur_tuple.add(j, final_wave_vol * dmg_vol[j]);
        }

        self.wave_sweep_cnt += config::AUDIO_SAMPLE_CLOCKS;
        if self.wave_length > 0 {
            self.wave_length -= config::AUDIO_SAMPLE_CLOCKS;
        }
    }

    // reset envelope, rate and length
    // channel num must be 0 or 1
    #[inline(always)]
    pub fn reset_square_channel(&mut self, channel_num: usize, bus: &Bus) {
        let snd_cur_cnt = bus.read_halfword_raw(0x62 + channel_num * 6, MemoryRegion::IO);
        let snd_cur_freq = bus.read_halfword_raw(0x64 + channel_num * 8, MemoryRegion::IO);
        self.square_envelope[channel_num] = snd_cur_cnt as u32 >> 0xc;
        self.square_length[channel_num] = (64 - (snd_cur_cnt as u32 & 0b111111)) << 16;
        self.square_rate[channel_num] = snd_cur_freq as u32 & 0b11111111111;
        self.square_sweep_cnt[channel_num] = 0;
        self.square_envelope_cnt[channel_num] = 0;
    }

    #[inline(always)]
    pub fn reset_wave_channel(&mut self, bus: &Bus) {
        self.wave_length = (256 - bus.read_byte_raw(0x72, MemoryRegion::IO) as u32) << 16;
        self.wave_rate = bus.read_halfword_raw(0x74, MemoryRegion::IO) as u32 & 0b11111111111;
        self.wave_sweep_cnt = 0;
    }
}
