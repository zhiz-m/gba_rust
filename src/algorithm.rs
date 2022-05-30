use std::hash::{Hasher, BuildHasher};
pub struct FastHasher {
    state: usize,
}

impl Hasher for FastHasher{
    fn finish(&self) -> u64 {
        self.state as u64
    }

    fn write(&mut self, _: &[u8]) {
        panic!("should not be called");
    }

    fn write_usize(&mut self, i: usize) {
        self.state = i * 1000000007;
    }
}

pub struct FastHashBuilder;

impl BuildHasher for FastHashBuilder{
    type Hasher = FastHasher;

    fn build_hasher(&self) -> Self::Hasher {
        FastHasher{state: 0}
    }
}


pub fn u8_search(data: &[u8], target: &[&[u8]]) -> Option<usize> {
    // slow brute force. optimise?
    for (num, str) in target.iter().enumerate() {
        let target_len = str.len();
        for i in 0..(data.len() >> 2){
            if (i<<2) + target_len <= data.len() && data[(i<<2)..(i<<2) + target_len] == **str{
                return Some(num);
            }
        }
    }
    None
}
