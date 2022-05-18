use std::{sync::mpsc::Receiver};

use crate::bus::Bus;

#[derive(Clone,Copy)]
pub enum KeyInput {
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
}

struct KeyBuffer(u16);

impl KeyBuffer {
    pub fn new() -> KeyBuffer {
        KeyBuffer(0b1111111111)
    }

    pub fn press_key(&mut self, key: KeyInput) {
        self.0 &= !(1 << key as u16);
    }

    pub fn release_key(&mut self, key: KeyInput) {
        self.0 |= 1 << key as u16;
    }
}

pub struct InputHandler {
    keybuf: KeyBuffer,
}

impl InputHandler {
    pub fn new() -> InputHandler {
        InputHandler {
            keybuf: KeyBuffer::new()
        }
    }

    pub fn process_input(&mut self, key_receiver: &Receiver<(KeyInput, bool)>, bus: &mut Bus) {
        while let Ok((key, is_pressed)) = key_receiver.try_recv() {
            if is_pressed{
                self.keybuf.press_key(key);
            }
            else{
                self.keybuf.release_key(key);
            }
        }
        bus.store_halfword(0x04000130, self.keybuf.0);
    }
}