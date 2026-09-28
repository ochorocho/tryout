use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("ui") => tryout::tui::run(&args[1..]),
        Some("__version") => {
            // Reading the marker through black_box keeps it in the binary even
            // if the linker were to drop an unreferenced static.
            let marker = std::hint::black_box(&tryout::DDEV_MARKER);
            print!(
                "{} {}",
                env!("CARGO_PKG_VERSION"),
                String::from_utf8_lossy(marker)
            );
            Ok(())
        }
        _ => {
            eprintln!("Usage: tryout ui [stop] [<project-dir>]");
            std::process::exit(64);
        }
    }
}
