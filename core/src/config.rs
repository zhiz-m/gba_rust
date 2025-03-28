use crate::bus::CartridgeType;

// number of clocks to execute for each call to sys::time::SystemTime::now().
// equal to one frame
pub const CPU_EXECUTION_INTERVAL_CLOCKS: u32 = 280896;

// number of microseconds that should pass after every CPU_EXECUTION_INTERVAL_CLOCKS clocks.
pub const CPU_EXECUTION_INTERVAL_US: u64 =
    1000000u64 * CPU_EXECUTION_INTERVAL_CLOCKS as u64 / (16 * 1024 * 1024);

// number of frames to pass before recording new FPS value
pub const FPS_RECORD_INTERVAL: u32 = 120;

pub const DEFAULT_CARTRIDGE_TYPE: CartridgeType = CartridgeType::Sram;

// note: the below memory addresses cannot be accessed by the user.

pub const FLASH64_MEM_START: usize = 0x0;
pub const FLASH64_MEM_END: usize = 0x10000;

pub const FLASH128_MEM_START: usize = 0x0;
pub const FLASH128_MEM_END: usize = 0x20000;

// 15 = sample rate of 32768
pub const AUDIO_SAMPLE_RATE_POW2: u32 = 16;
pub const AUDIO_SAMPLE_RATE: u32 = 1 << AUDIO_SAMPLE_RATE_POW2;
pub const AUDIO_SAMPLE_CLOCKS_POW2: u32 = 24 - AUDIO_SAMPLE_RATE_POW2;
pub const AUDIO_SAMPLE_CLOCKS: u32 = 1 << AUDIO_SAMPLE_CLOCKS_POW2;
pub const AUDIO_SAMPLE_CHUNKS: usize = 1024;

pub const NUM_SAVE_STATES: usize = 5;
pub const SAVE_STATE_SIZE: usize = 128 * 1024 * NUM_SAVE_STATES;

// number of frames to pass before rendering in speedup mode
pub const FRAME_RENDER_INTERVAL_SPEEDUP: u32 = 8;

const TIMER_CLOCK_INTERVAL_POW2: u32 = 7;
pub const TIMER_CLOCK_INTERVAL_CLOCKS: u32 = 1 << TIMER_CLOCK_INTERVAL_POW2;
const DMA_CHECK_INTERVAL_POW2: u32 = 3;
pub const DMA_CHECK_INTERVAL_CLOCKS: u32 = 1 << DMA_CHECK_INTERVAL_POW2;

/*#[cfg(feature="fast_cpu")]
// WARNING: UNSTABLE
pub const CPU_ITERATIONS_PER_SIMULATION: usize = 1;

#[cfg(not(feature="fast_cpu"))]
pub const CPU_ITERATIONS_PER_SIMULATION: usize = 1;*/

// lower is more accurate, higher allows faster emulation.
pub const CPU_HALT_SLEEP_CYCLES: u32 = 32;
