use crate::{bus::Bus, config};

#[derive(Clone, Copy)]
pub enum KeyInput {
    // GBA official keys
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,

    // Emulator introduced keys
    Speedup = 10,
    Save0 = 11,
    Save1 = 12,
    Save2 = 13,
    Save3 = 14,
    Save4 = 15,
}

impl TryFrom<u8> for KeyInput {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => KeyInput::A,
            1 => KeyInput::B,
            2 => KeyInput::Select,
            3 => KeyInput::Start,
            4 => KeyInput::Right,
            5 => KeyInput::Left,
            6 => KeyInput::Up,
            7 => KeyInput::Down,
            8 => KeyInput::R,
            9 => KeyInput::L,
            10 => KeyInput::Speedup,
            11 => KeyInput::Save0,
            12 => KeyInput::Save1,
            13 => KeyInput::Save2,
            14 => KeyInput::Save3,
            15 => KeyInput::Save4,
            _ => return Err(()),
        })
    }
}

impl From<KeyInput> for u8 {
    fn from(val: KeyInput) -> Self {
        val as u8
    }
}

struct KeyBuffer(u16);

impl KeyBuffer {
    pub fn new() -> KeyBuffer {
        KeyBuffer(0b1111111111)
    }

    // assumes key is a GBA official key, eg (key as u32 <= 9)
    pub fn press_key(&mut self, key: KeyInput) {
        self.0 &= !(1 << key as u16);
    }

    // assumes key is a GBA official key, eg (key as u32 <= 9)
    pub fn release_key(&mut self, key: KeyInput) {
        self.0 |= 1 << key as u16;
    }
}

pub struct InputHandler {
    keybuf: KeyBuffer,

    // speedup state: true means emulator is in speedup mode
    pub prev_speedup_state: bool,
    pub cur_speedup_state: bool,

    pub save_requested: [bool; config::NUM_SAVE_STATES],
}

impl InputHandler {
    pub fn new() -> InputHandler {
        InputHandler {
            keybuf: KeyBuffer::new(),
            prev_speedup_state: false,
            cur_speedup_state: false,
            save_requested: [false; config::NUM_SAVE_STATES],
        }
    }

    #[inline(always)]
    pub fn process_key(&mut self, key: KeyInput, is_pressed: bool) {
        match key {
            KeyInput::Speedup => {
                self.cur_speedup_state = is_pressed;
            }
            KeyInput::Save0
            | KeyInput::Save1
            | KeyInput::Save2
            | KeyInput::Save3
            | KeyInput::Save4 => {
                self.save_requested[key as usize - KeyInput::Save0 as usize] = is_pressed;
            }
            _ => {
                if is_pressed {
                    self.keybuf.press_key(key);
                } else {
                    self.keybuf.release_key(key);
                }
            }
        }
    }

    // CR-someday zhizma: use the below, decouple saves/speedup from keys
    #[inline(always)]
    pub fn process_speedup(&mut self, enable: bool) {
        self.cur_speedup_state = enable
    }

    #[inline(always)]
    pub fn process_save(&mut self, index: usize) {
        self.save_requested[index] = true
    }

    // must be called before processing keys for each frame
    #[inline(always)]
    pub fn frame_preprocess(&mut self) {
        self.prev_speedup_state = self.cur_speedup_state;
    }

    #[inline(always)]
    pub fn commit(&self, bus: &mut Bus) {
        bus.store_halfword(0x04000130, self.keybuf.0);
    }

    /*pub fn process_input(&mut self, key_receiver: &Receiver<(KeyInput, bool)>, bus: &mut Bus) {
        self.prev_speedup_state = self.cur_speedup_state;
        while let Ok((key, is_pressed)) = key_receiver.try_recv() {
            match key {
                KeyInput::Speedup => {
                    self.cur_speedup_state = is_pressed;
                }
                KeyInput::Save0 | KeyInput::Save1 | KeyInput::Save2 | KeyInput::Save3 | KeyInput::Save4 => {
                    self.save_requested[key as usize - KeyInput::Save0 as usize] = is_pressed;
                },
                _ => {
                    if is_pressed{
                        self.keybuf.press_key(key);
                    }
                    else{
                        self.keybuf.release_key(key);
                    }
                }
            }

        }
        bus.store_halfword(0x04000130, self.keybuf.0);
    }*/
}
