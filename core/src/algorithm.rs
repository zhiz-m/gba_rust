use std::hash::{BuildHasher, Hasher};

use crate::{cpu::CPU, bus::Bus, config::CPU_ARM_CACHE_SIZE_POW2};
pub struct FastHasher {
    state: usize,
}

impl Hasher for FastHasher {
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

impl BuildHasher for FastHashBuilder {
    type Hasher = FastHasher;

    fn build_hasher(&self) -> Self::Hasher {
        FastHasher { state: 0 }
    }
}

pub struct DumbHashMap<U: Clone>
{
    data: Vec<Option<(u32, U)>>,
}

impl<U: Clone + Copy> DumbHashMap<U>{
    pub fn new() -> DumbHashMap<U>{
        DumbHashMap { 
            data: vec![None; 1 << CPU_ARM_CACHE_SIZE_POW2],
        }
    }

    pub fn get(&self, key: &u32) -> Option<U> {
        if let Some(item) = &self.data[DumbHashMap::<U>::hash(key)]{
            if item.0 << 4 == key << 4{
                return Some(item.1);
            }
        }
        return None;
    }

    pub fn insert(&mut self, key: u32, val: U){
        self.data[DumbHashMap::<U>::hash(&key)] = Some((key, val));
    }

    fn hash(key: &u32) -> usize {
        ((key & 0xfffffff) >> (28 - CPU_ARM_CACHE_SIZE_POW2)) as usize
    }
}

pub fn u8_search(data: &[u8], target: &[&[u8]]) -> Option<usize> {
    // slow brute force. optimise?
    for (num, str) in target.iter().enumerate() {
        let target_len = str.len();
        for i in 0..(data.len() >> 2) {
            if (i << 2) + target_len <= data.len() && data[(i << 2)..(i << 2) + target_len] == **str
            {
                return Some(num);
            }
        }
    }
    None
}
