
mod frontend;
mod config;

use clap::Parser;
use frontend::Frontend;
use gba_core::{GBA, ScreenBuffer, SAVE_STATE_SIZE};

use std::{env, thread, sync::mpsc, fs::{File, read, self}, io::{Read, BufReader}, path::Path};

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
    
    // screen buffer
    let (tx1, rx1) = mpsc::channel();
    
    
    let (tx2, rx2) = mpsc::channel();
    
    // audio
    let (tx3, rx3) = mpsc::channel();

    

    // fps
    let (tx4, rx4) = mpsc::channel();
    
    let screenbuf_handler = move |screenbuf: ScreenBuffer|{
        if let Err(why) = tx1.send(screenbuf){
            println!("   screenbuf sending error: {}", why.to_string());
        }
    };
    
    let audio_handler = move |buf: &[Vec<f32>]|{
        //tx3.send((0f32,0f32)).unwrap();
        for j in 0..buf[0].len(){
            tx3.send((buf[0][j], buf[1][j])).unwrap();
        }
    };

    let bios_bin = read(bios_path).expect("did not find BIOS file");
    let rom_bin = read(&cli.rom_path).expect("did not find ROM");
    let rom_save_path = match cli.rom_save_path {
        Some(path) => path.to_string(),
        None => {
            let save_state_dir = Path::new(&cli.rom_path).parent().unwrap().to_str().expect("invalid rom path").to_string() + config::SAVE_FILE_DIR;
            fs::create_dir_all(&save_state_dir).unwrap();
            let rom_path_filename = Path::new(&cli.rom_path).file_name().unwrap().to_str().unwrap().to_string();
            println!("save_state_dir: {}, rom_path_filename: {}", save_state_dir, rom_path_filename);
            let rom_save_path = if rom_path_filename.contains("."){
                let pos = rom_path_filename.rfind(".").unwrap();
                if pos != 0{
                    format!("{}{}", &rom_path_filename[0..pos], config::SAVE_FILE_SUF)
                }
                else{
                    format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
                }
            }
            else{
                format!("{}{}", &rom_path_filename, config::SAVE_FILE_SUF)
            };
            save_state_dir + "/" + &rom_save_path
        }
    }; 
    println!("rom save path: {}", rom_save_path);
    // read save path into save_state
    let save_state = if Path::new(&rom_save_path).exists() {
        let mut save_state = vec![0; SAVE_STATE_SIZE];
        let mut reader = BufReader::new(File::open(&rom_save_path).unwrap());
        reader.read(&mut save_state).unwrap();
        Some(save_state)
    }
    else{
        None
    };

    let save_state_handler = move |save_state: &[u8]|{
        fs::write(&rom_save_path, save_state).unwrap();
        println!("save written to {}", &rom_save_path);
    };

    let mut frontend = Frontend::new("gba_rust frontend".to_string(), rx1, tx2, rx3, rx4);
    let mut gba = GBA::new(&bios_bin, &rom_bin, save_state, cli.save_state_bank, cli.cartridge_type_str.as_deref(), Box::new(save_state_handler), Box::new(screenbuf_handler), rx2, Box::new(audio_handler), frontend.get_sample_rate(), tx4);

    thread::spawn(move || {
        gba.start().unwrap();
    });

    frontend.start().unwrap();
}
