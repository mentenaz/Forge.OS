use std::process::ExitCode;

use surface_preview::{png, scene};

fn main() -> ExitCode {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "preview.png".into());
    let rgb = scene::to_rgb(&scene::render());
    let Some(file) = png::encode(scene::WIDTH as u32, scene::HEIGHT as u32, &rgb) else {
        eprintln!("the scene is not a valid picture");
        return ExitCode::FAILURE;
    };
    match std::fs::write(&path, file) {
        Ok(()) => {
            println!("wrote {path} ({}x{})", scene::WIDTH, scene::HEIGHT);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cannot write {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
