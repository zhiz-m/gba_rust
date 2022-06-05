
use std::{sync::mpsc::Sender, collections::VecDeque};

use crate::{
    bus::Bus,
    config
};
use rubato::{Resampler, FftFixedInOut};

// StereoTuple.0 is right, StereoTuple.1 is left
struct StereoTuple(Option<i16>, Option<i16>);
impl StereoTuple {
    pub fn new() -> StereoTuple{
        StereoTuple(None, None)
    }
    pub fn add(&mut self, channel: usize, val: i16){
        match channel {
            0 => self.0 = match self.0 {
                None => Some(val),
                Some(cur) => Some(cur + val),
            },
            1 => self.1 = match self.1 {
                None => Some(val),
                Some(cur) => Some(cur + val),
            },
            _ => unreachable!(),
        }
    }
    pub fn add_bias(&mut self, channel: usize, val: i16){
        match channel {
            0 => self.0 = match self.0 {
                None => None,
                Some(cur) => Some(cur + val),
            },
            1 => self.1 = match self.1 {
                None => None,
                Some(cur) => Some(cur + val),
            },
            _ => unreachable!(),
        }
    }
    pub fn multiply(&mut self, channel: usize, val: i16){
        match channel {
            0 => self.0 = match self.0 {
                None => None,
                Some(cur) => Some(cur * val),
            },
            1 => self.1 = match self.1 {
                None => None,
                Some(cur) => Some(cur * val),
            },
            _ => unreachable!(),
        }
    }
    pub fn clip(&mut self) {
        self.0 = match self.0 {
            None => None,
            Some(val) => Some(std::cmp::max(0, std::cmp::min(0x3ff, val))),
        };
        self.1 = match self.1 {
            None => None,
            Some(val) => Some(std::cmp::max(0, std::cmp::min(0x3ff, val))),
        };
    }
}

pub struct APU {
    //  ------- square sound channels
    square_length: [u32; 2],
    square_rate: [u32; 2],
    square_envelope: [u32; 2],

    // counts number of clock cycles
    square_sweep_cnt: [u32; 2], 
    square_envelope_cnt: [u32; 2],

    pub square_disable: [bool; 2],

    // -------- direct sound (DMA) channels
    pub direct_sound_fifo: Vec<VecDeque<i8>>,
    pub direct_sound_fifo_cur: [i8; 2],
    pub direct_sound_timer: [Option<usize>; 2],

    sound_in_buff: Vec<Vec<f32>>,
    sound_out_buff: Vec<Vec<f32>>,
    sampler: FftFixedInOut<f32>,
    audio_sender: Sender<(f32, f32)>,

    //t: Duration,
}

impl APU {
    pub fn new(sample_rate_output: usize, audio_sender: Sender<(f32, f32)>) -> APU {
       /* let params = InterpolationParameters{
            sinc_len: 256,
            f_cutoff: 0.95,
            oversampling_factor: 128,
            interpolation: InterpolationType::Cubic,
            window: rubato::WindowFunction::Hann,
        };
        let sampler = SincFixedIn::new(sample_rate_output as f64 / 32768f64, sample_rate_output as f64 / 32768f64, params, 1024, 2).unwrap();
        */
        let sampler = FftFixedInOut::new(config::AUDIO_SAMPLE_RATE as usize, sample_rate_output, 1024, 2).unwrap();
        println!("sampler input required size: {}", sampler.input_frames_next());
        APU {  
            square_length: [0; 2],
            square_rate: [0; 2],
            square_envelope: [0; 2],

            square_sweep_cnt: [0; 2],
            square_envelope_cnt: [0; 2],

            square_disable: [false; 2],

            direct_sound_fifo: vec![VecDeque::<i8>::with_capacity(64); 2],
            direct_sound_fifo_cur: [0; 2],
            direct_sound_timer: [None; 2],

            sound_in_buff: sampler.input_buffer_allocate(),
            sound_out_buff: sampler.output_buffer_allocate(),
            sampler,
            audio_sender,

            //t: SystemTime::now().duration_since(UNIX_EPOCH).unwrap()

        }
    }

    // called every config::AUDIO_SAMPLE_CLOCKS clocks
    pub fn clock(&mut self, bus: &Bus) {
        self.square_disable[0] = false;
        self.square_disable[1] = false;
        let mut cur_tuple = StereoTuple::new();
        let snd_stat = bus.read_byte_raw(0x04000084);
        if (snd_stat >> 7) & 1 > 0{
            // sound enabled
            let snd_dmg_cnt = bus.read_halfword_raw(0x04000080);
            let snd_ds_cnt = bus.read_halfword_raw(0x04000082);
            
            let dmg_vol = [snd_dmg_cnt as i16 & 0b111, (snd_dmg_cnt >> 4) as i16 & 0b111];

            // square channels
            for i in 0..2 {
                let enable_right_left = [(snd_dmg_cnt >> (8 + i)) & 1 > 0, (snd_dmg_cnt >> (12 + i)) & 1 > 0];
                // sound is not enabled on any channel (left or right)
                if !enable_right_left[0] && !enable_right_left[1] {
                    continue;
                }
                let snd_cur_freq = bus.read_halfword(0x04000064 + 8 * i);

                if (snd_cur_freq >> 0xe) & 1 > 0 && self.square_length[i] == 0 {
                    continue;
                }
                // process sweep
                if i == 0{
                    let snd_sweep = bus.read_byte(0x04000060);
                    let sweep_cnt_hit = ((snd_sweep as u32 >> 4) & 0b111) << 17;
                    if sweep_cnt_hit != 0{
                        if self.square_sweep_cnt[i] >= sweep_cnt_hit {
                            let sweep_num = snd_sweep & 0b111;
                            let rate_delta = self.square_rate[i] >> sweep_num;
                            if (snd_sweep >> 3) & 1 > 0 {
                                self.square_rate[i] -= rate_delta;
                            }
                            // would overflow, disable current channel
                            else if 2048 - self.square_rate[i] <= rate_delta {
                                self.square_disable[i] = true;
                                continue;
                            }
                            else{
                                self.square_rate[i] += rate_delta;
                            }
                            self.square_sweep_cnt[i] = 0;
                        }
                    }
                }
                let snd_cur_cnt = bus.read_halfword(0x04000062 + i * 6);

                // process envelope changes
                let envelope_cnt_hit = ((snd_cur_cnt as u32 >> 8) & 0b111) << 18;
                let envelope_increase = (snd_cur_cnt >> 0xb) & 1 > 0;
                if envelope_cnt_hit != 0 && !((envelope_increase && self.square_envelope[i] == 0b1111) || (!envelope_increase && self.square_envelope[i] == 0)) {
                    if self.square_envelope_cnt[i] >= envelope_cnt_hit {
                        if envelope_increase {
                            self.square_envelope[i] += 1;
                        }
                        else{
                            self.square_envelope[i] -= 1;
                        }
                        self.square_envelope_cnt[i] = 0;
                    }
                    self.square_envelope_cnt[i] += config::AUDIO_SAMPLE_CLOCKS;
                }

                // process duty cycle
                let period_clocks = (2048 - self.square_rate[i]) << 7;
                let active_clocks = match (snd_cur_cnt >> 6) & 0b11{
                    0b00 => period_clocks >> 3,
                    0b01 => period_clocks >> 2,
                    0b10 => period_clocks >> 1,
                    0b11 => (period_clocks >> 2) * 3,
                    _ => unreachable!(),
                };

                // sound channels 
                for j in 0..2 {
                    if !enable_right_left[j]{
                        continue;
                    }
                    let final_square_vol = match snd_ds_cnt & 0b11 {
                        0b00 => self.square_envelope[i] >> 2,
                        0b01 => self.square_envelope[i] >> 1,
                        0b10 => self.square_envelope[i],
                        0b11 => panic!("sound channel 1-4 has a volume of 0b11: forbidden"),
                        _ => unreachable!(),
                    } as i16;
                    if self.square_sweep_cnt[i] % period_clocks < active_clocks {
                        cur_tuple.add(j, final_square_vol * dmg_vol[j]);
                    }
                    else{
                        cur_tuple.add(j, -final_square_vol * dmg_vol[j]);
                    }
                }

                self.square_sweep_cnt[i] += config::AUDIO_SAMPLE_CLOCKS;
                if self.square_length[i] > 0 {
                    self.square_length[i] -= 1;
                }
            }
            
            // Direct Sound
            for i in 0..2 {
                let enable_right_left = [(snd_ds_cnt >> (8 + 4 * i)) & 1 > 0, (snd_ds_cnt >> (9 + 4 * i)) & 1 > 0];
                if !enable_right_left[0] && !enable_right_left[1] {
                    continue;
                }
                // sound right and left channels
                for j in 0..2 {
                    if !enable_right_left[j]{
                        continue;
                    }
                    let final_sample = match (snd_ds_cnt >> (2 + j)) & 1 {
                        0 => self.direct_sound_fifo_cur[i] >> 1,
                        1 => self.direct_sound_fifo_cur[i],
                        _ => unreachable!()
                    };
                    if final_sample as i16 != 0{
                        //println!("playing from direct sound: {:#x}, ds_cnt: {:#018b}, channel: {}, snd_bias: {:#018b}", final_sample, snd_ds_cnt, i, bus.read_halfword(0x04000088));
                    }
                    cur_tuple.add(j, final_sample as i16);
                }
            }

            // process volume
            //cur_tuple.multiply(0, snd_dmg_cnt as i16 & 0b111);
            //cur_tuple.multiply(1, (snd_dmg_cnt >> 4) as i16 & 0b111);

            // process bias
            let snd_bias = bus.read_word_raw(0x04000088);
            let bias = (snd_bias >> 0) & 0b1111111111;
            cur_tuple.add_bias(0, bias as i16);
            cur_tuple.add_bias(1, bias as i16);

            // clip values into range [0, 0x3ff]
            cur_tuple.clip();
        }

        // output channel 0 is left not right
        self.sound_in_buff[1].push(match cur_tuple.0 {
            None => 0f32,
            Some(val) => (val as f32 - 512.) / 512.,
        });
        self.sound_in_buff[0].push(match cur_tuple.1 {
            None => 0f32,
            Some(val) => (val as f32 - 512.)  / 512.,
        });

        if self.sound_in_buff[0].len() == self.sampler.input_frames_next() {
            self.sampler.process_into_buffer(&self.sound_in_buff, &mut self.sound_out_buff, None).unwrap();
            for j in 0..self.sound_out_buff[0].len(){
                self.audio_sender.send((self.sound_out_buff[0][j], self.sound_out_buff[1][j])).unwrap();
            }
            self.sound_in_buff[0].clear();
            self.sound_in_buff[1].clear();
            self.sound_out_buff[0].clear();
            self.sound_out_buff[1].clear();
        }
    }

    // reset envelope, rate and length
    // channel num must be 0 or 1
    pub fn reset_square_channel(&mut self, channel_num: usize, bus: &Bus) {
        let snd_cur_cnt = bus.read_halfword(0x04000062 + channel_num * 6);
        let snd_cur_freq = bus.read_halfword(0x04000064 + channel_num * 8);
        self.square_envelope[channel_num] = snd_cur_cnt as u32 >> 0xc;
        self.square_length[channel_num] = snd_cur_cnt as u32 & 0b111111;
        self.square_rate[channel_num] = snd_cur_freq as u32 & 0b11111111111;
        self.square_sweep_cnt[channel_num] = 0;
        self.square_envelope_cnt[channel_num] = 0;
    }
}