mod config;
mod frontend;
mod logger;

use clap::Parser;
use frontend::Frontend;
use gba_sim::StateLogger;
use log::{info, warn};

use std::{
    env,
    fs::{self, read},
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::logger::init_logger;

#[derive(Parser)]
#[clap(about = "GBA emulator written in Rust")]
struct Arguments {
    /// Path to .gba ROM
    #[clap(short = 'o', long)]
    rom_path: String,

    /// (Optional) Path to .rustsav save file for ROM. Leave empty to use the default save directory, which is relative to the ROM path.
    #[clap(short = 's', long)]
    rom_save_path: Option<String>,

    /// (Optional) Type of cartridge: [SRAM_V, FLASH_V, FLASH512_V, FLASH1M_V, EEPROM_V]. Leave empty for automatic detection.
    #[clap(short, long)]
    cartridge_type_str: Option<String>,

    /// Save bank to load from (integer; [0,4])
    #[clap(short = 'b', long)]
    save_state_bank: Option<usize>,

    /// Name of the preferred audio device
    #[clap(short = 'a', long)]
    audio_device: Option<String>,

    /// Path to save sim state
    #[clap(short = 't', long)]
    sim_state_path: Option<String>,
}

fn main() {
    init_logger().expect("failed to init logger");

    let cli = Arguments::parse();
    //let rom_path = env::args().nth(1).expect("first argument must be the path to a .gba ROM fle");
    //let rom_save_path = env::args().nth(2);
    //let cartridge_type_str = env::args().nth(3);
    let bios_path =
        env::var("GBA_RUST_BIOS_PATH").expect("Env variable GBA_RUST_BIOS_PATH not found");

    // screen buffer
    let (tx1, rx1) = mpsc::channel();

    let (tx2, rx2) = mpsc::channel();

    // audio
    let (tx3, rx3) = mpsc::channel();

    // fps
    let (tx4, rx4) = mpsc::channel();

    // finish
    let (tx5, rx5) = mpsc::channel();

    let bios_bin = read(bios_path).expect("did not find BIOS file");
    let rom_bin = read(&cli.rom_path).expect("did not find ROM");
    let rom_save_path = match cli.rom_save_path {
        Some(path) => path,
        None => {
            let save_state_dir = Path::new(&cli.rom_path)
                .parent()
                .unwrap()
                .to_str()
                .expect("invalid rom path")
                .to_string()
                + config::SAVE_FILE_DIR;
            fs::create_dir_all(&save_state_dir).unwrap();
            let rom_path_filename = Path::new(&cli.rom_path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            info!(
                "save_state_dir: {}, rom_path_filename: {}",
                save_state_dir, rom_path_filename
            );
            let rom_save_path = if rom_path_filename.contains('.') {
                let pos = rom_path_filename.rfind('.').unwrap();
                if pos != 0 {
                    format!("{}{}", &rom_path_filename[0..pos], config::SAVE_FILE_SUF)
                } else {
                    format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
                }
            } else {
                format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
            };
            save_state_dir + "/" + &rom_save_path
        }
    };
    info!("rom save path: {}", rom_save_path);
    // read save path into save_state
    let save_state = fs::read(&rom_save_path)
        .map(|bin| gba_core::marshall_save_state(&bin))
        .ok();

    let mut frontend = Frontend::new(
        "gba_rust frontend".to_string(),
        cli.audio_device.as_deref(),
        rx1,
        tx2,
        rx3,
        rx4,
    );
    let mut gba = gba_core::GBA::new(
        &bios_bin,
        &rom_bin,
        save_state.clone(),
        cli.save_state_bank,
        cli.cartridge_type_str.as_deref(),
        frontend.get_sample_rate(),
    );

    let thread = thread::spawn(move || {
        let save = match (save_state, cli.save_state_bank) {
            (Some(save_state), Some(save_state_bank)) => Some((save_state, save_state_bank)),
            _ => None,
        };
        let mut state_logger = StateLogger::new(cli.rom_path, save);
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        gba.init(current_time);
        state_logger.init(current_time);
        loop {
            let frame = gba.total_frames_passed();
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            state_logger.log_frame(frame, current_time);
            let sleep_micros = gba.process_frame(current_time).unwrap();
            thread::sleep(Duration::from_micros(sleep_micros));

            // video
            if let Some(screen_buffer) = gba.get_screen_buffer() {
                if let Err(why) = tx1.send(screen_buffer.clone()) {
                    warn!("   screenbuf sending error: {}", why);
                }
            }

            // audio
            if let Some(it) = gba.get_sound_buffer() {
                it.for_each(|data| tx3.send(data).unwrap());
                gba.reset_sound_buffer();
            }

            // saves
            if let Some(save_state) = gba.get_updated_save_state() {
                fs::write(&rom_save_path, save_state[..].concat()).unwrap();
                info!("save written to {}", &rom_save_path);
            }

            // fps
            if let Some(fps) = gba.get_fps() {
                tx4.send(fps).unwrap();
            }

            gba.input_frame_preprocess();

            // input
            while let Ok((key, is_pressed)) = rx2.try_recv() {
                gba.process_key(key, is_pressed);
                state_logger.log_key_input_for_current_frame(key, is_pressed);
            }

            //info!("process frame");
            if let Ok(()) = rx5.try_recv() {
                let state = state_logger.finalize();
                if let Some(sim_state_path) = cli.sim_state_path {
                    gba_sim::sim::save_state(&state, &sim_state_path);
                }

                break;
            }
        }
    });

    frontend.start().unwrap();
    tx5.send(()).unwrap();
    thread.join().unwrap()
}
