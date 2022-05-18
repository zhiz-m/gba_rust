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

pub struct KeyBuffer {
    key_bitmap: u16,
}

impl KeyBuffer {
    pub fn new() -> KeyBuffer {
        KeyBuffer { 
            key_bitmap: 0b1111111111,
        }
    }

    pub fn press_key(&mut self, key: KeyInput) {
        self.key_bitmap &= !(key as u16);
    }

    pub fn release_key(&mut self, key: KeyInput) {
        self.key_bitmap |= key as u16;
    }

    pub fn get_key_input(&self) -> u16 {
        self.key_bitmap
    }
}
