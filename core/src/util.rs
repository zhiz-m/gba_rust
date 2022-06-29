use crate::config;

pub fn marshall_save_state(bin: &[u8]) -> Vec<Vec<u8>> {
    bin.chunks(bin.len() / config::NUM_SAVE_STATES)
        .map(|x| x.to_vec())
        .collect()
}
