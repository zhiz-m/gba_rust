use clap::Parser;
use gba_sim::sim::{drive_gba_from_state, load_state};

#[derive(Parser)]
#[clap(about = "GBA emulator sim")]
struct Arguments {
    /// Path to load sim state
    #[clap(short = 't', long)]
    sim_state_path: String,

    /// Path to save final image buffer
    #[clap(short = 'b', long)]
    image_buffer_path: Option<String>,
}

fn main() {
    let cli = Arguments::parse();
    let state = load_state(&cli.sim_state_path);
    let img = drive_gba_from_state(state);
    if let Some(path) = cli.image_buffer_path {
        img.save(&path).unwrap()
    }
}
