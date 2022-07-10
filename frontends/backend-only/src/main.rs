
use clap::Parser;
use gba_core::{self, ScreenBuffer, KeyInput};

use std::{
    env,
    fs::{self, read, File},
    io::{BufReader, Read},
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread, time::{SystemTime, UNIX_EPOCH, Duration},
};

#[derive(Parser)]
#[clap(about = "GBA emulator written in Rust")]
struct Arguments {
    /// Path to .gba ROM
    #[clap(short = 'o', long)]
    rom_path: String,

    /// Path to .rustsav save file for ROM
    #[clap(short = 's', long)]
    rom_save_path: Option<String>,

    /// Type of cartridge: [SRAM_V, FLASH_V, FLASH512_V, FLASH1M_V, EEPROM_V]
    #[clap(short, long)]
    cartridge_type_str: Option<String>,

    /// Save bank to load from
    #[clap(short = 'b', long)]
    save_state_bank: Option<usize>,
}

fn main() {
    let cli = Arguments::parse();
    //let rom_path = env::args().nth(1).expect("first argument must be the path to a .gba ROM fle");
    //let rom_save_path = env::args().nth(2);
    //let cartridge_type_str = env::args().nth(3);
    let bios_path = "./extern/GBA/gba_bios.bin";

    let bios_bin = read(bios_path).expect("did not find BIOS file");
    let rom_bin = read(&cli.rom_path).expect("did not find ROM");

    let mut gba = gba_core::GBA::new(
        &bios_bin,
        &rom_bin,
        None,
        cli.save_state_bank,
        cli.cartridge_type_str.as_deref(),
        65536,
    );
    std::mem::drop(bios_bin);
    std::mem::drop(rom_bin);
    gba.init(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64);
    //gba.input_frame_preprocess();
    //gba.process_key(KeyInput::Speedup, true);
    loop {
        let sleep_micros = gba.process_frame(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64).unwrap();
        thread::sleep(Duration::from_micros(sleep_micros));

        // audio
        if let Some(it) = gba.get_sound_buffer() {
            gba.reset_sound_buffer();
        }

        gba.input_frame_preprocess();
    }
}
