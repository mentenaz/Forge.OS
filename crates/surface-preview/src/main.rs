use std::process::ExitCode;

use surface_preview::{png, scene};

/// `surface-preview [OUTPUT.png [WIDTH HEIGHT]]`
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "preview.png".into());
    let size = |arg: Option<String>, default: usize| match arg {
        None => Some(default),
        Some(text) => text.parse().ok().filter(|&n| n > 0),
    };
    let (Some(width), Some(height)) = (
        size(args.next(), scene::WIDTH),
        size(args.next(), scene::HEIGHT),
    ) else {
        eprintln!("usage: surface-preview [OUTPUT.png [WIDTH HEIGHT]]");
        return ExitCode::FAILURE;
    };
    let Some(words) = scene::render(width, height) else {
        eprintln!("{width}x{height} is not a size the preview can draw");
        return ExitCode::FAILURE;
    };
    let rgb = scene::to_rgb(&words);
    let Some(file) = png::encode(width as u32, height as u32, &rgb) else {
        eprintln!("the scene is not a valid picture");
        return ExitCode::FAILURE;
    };
    match std::fs::write(&path, file) {
        Ok(()) => {
            println!("wrote {path} ({width}x{height})");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cannot write {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
