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