
use crate::bus::CartridgeType;

// number of clocks to execute for each call to sys::time::SystemTime::now().
// equal to one frame 
pub const CPU_EXECUTION_INTERVAL_CLOCKS: u32 = 280896;  

// number of nanoseconds that should pass after every CPU_EXECUTION_INTERVAL_CLOCKS clocks. 
pub const CPU_EXECUTION_INTERVAL_NS: u32 = 1000000000 / ( 16 * 1024 * 1024 / CPU_EXECUTION_INTERVAL_CLOCKS);

// number of frames to pass before recording new FPS value
pub const FPS_RECORD_INTERVAL: u32 = 60;

pub const DEFAULT_CARTRIDGE_TYPE: CartridgeType = CartridgeType::SRAM;

// note: the below memory addresses cannot be accessed by the user. 

pub const FLASH64_MEM_START: usize = 0x0e000000;
pub const FLASH64_MEM_END: usize = 0x0e010000;

pub const FLASH128_MEM_START: usize = 0x0e000000;
pub const FLASH128_MEM_END: usize = 0x0e020000;

// 15 = sample rate of 32768
pub const AUDIO_SAMPLE_RATE_POW2: u32 = 16;
pub const AUDIO_SAMPLE_RATE: u32 = 1 << AUDIO_SAMPLE_RATE_POW2;
pub const AUDIO_SAMPLE_CLOCKS_POW2: u32 = 24 - AUDIO_SAMPLE_RATE_POW2;
pub const AUDIO_SAMPLE_CLOCKS: u32 = 1 << AUDIO_SAMPLE_CLOCKS_POW2;

pub const NUM_SAVE_STATES: usize = 5;