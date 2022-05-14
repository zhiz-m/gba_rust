mod bus;
mod cpu;

use bus::Bus;
use cpu::Cpu;

fn main() {
    let bus = Bus::new();
    let cpu = Cpu::new();
}
