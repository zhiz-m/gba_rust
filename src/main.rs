
mod frontend;

use clap::Parser;
use frontend::Frontend;
use gba_core::GBA;

use std::{env, thread, sync::mpsc};

#[derive(Parser)]
#[clap(about="GBA emulator written in Rust")]
struct Arguments{
    /// Path to .gba ROM
    #[clap(short='o',long,)]
    rom_path: String,

    /// Path to .rustsav save file for ROM
    #[clap(short='s',long,)]
    rom_save_path: Option<String>,

    /// Type of cartridge: [SRAM_V, FLASH_V, FLASH512_V, FLASH1M_V, EEPROM_V]
    #[clap(short,long)]
    cartridge_type_str: Option<String>,

    /// Save bank to load from
    #[clap(short='b',long)]
    save_state_bank: Option<usize>,
}

fn main() {
    let cli = Arguments::parse();
    //let rom_path = env::args().nth(1).expect("first argument must be the path to a .gba ROM fle");
    //let rom_save_path = env::args().nth(2);
    //let cartridge_type_str = env::args().nth(3);
    let bios_path = "./extern/GBA/gba_bios.bin";

    let (tx, rx) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();
    
    // audio
    let (tx3, rx3) = mpsc::channel();

    // fps
    let (tx4, rx4) = mpsc::channel();

    let mut frontend = Frontend::new("gba_rust frontend".to_string(), rx, tx2, rx3, rx4);
    let mut gba = GBA::new(&bios_path, &cli.rom_path, cli.rom_save_path.as_deref(), cli.save_state_bank, cli.cartridge_type_str.as_deref(), tx, rx2, tx3, frontend.get_sample_rate(), tx4);

    thread::spawn(move || {
        gba.start().unwrap();
    });

    frontend.start().unwrap();
}
