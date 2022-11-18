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

    // must be called before processing keys for each frame
    pub fn frame_preprocess(&mut self) {
        self.prev_speedup_state = self.cur_speedup_state;
    }

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
