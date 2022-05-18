
// number of clocks to execute for each call to sys::time::SystemTime::now(). 
pub const CPU_EXECUTION_INTERVAL_CLOCKS: u64 = 16 * 1024; 

// number of nanoseconds that should pass after every CPU_EXECUTION_INTERVAL_CLOCKS clocks. 
pub const CPU_EXECUTION_INTERVAL_NS: u64 = 1000000000 / CPU_EXECUTION_INTERVAL_CLOCKS;