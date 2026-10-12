#[cfg(windows)]
mod windows_capture;

#[cfg(windows)]
fn main() {
    use clap::Parser;
    if let Err(error) = windows_capture::run(capture_spike::args::Args::parse()) {
        eprintln!("capture-spike: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("capture-spike only runs on Windows");
    std::process::exit(1);
}
