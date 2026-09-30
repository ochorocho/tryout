fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        // The TUI's own entry, with a project path: `ui <root>`, `ui stop <root>`
        // and the session server's `ui server <root>`. A bare `ui` / `ui stop` is
        // the verb, resolved against DDEV's project like every other.
        Some("ui") if tui_direct(&args[1..]) => match tryout::tui::run(&args[1..]) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("Error: {e:#}");
                1
            }
        },
        Some("__version") => {
            // Read through black_box: the marker has to survive into the binary.
            let marker = std::hint::black_box(&tryout::DDEV_MARKER);
            print!(
                "{} {}",
                env!("CARGO_PKG_VERSION"),
                String::from_utf8_lossy(marker)
            );
            0
        }
        _ => tryout::cli::main(args),
    };
    std::process::exit(code);
}

fn tui_direct(rest: &[String]) -> bool {
    match rest {
        [first, ..] if first == "server" || first.starts_with('/') => true,
        [stop, path] if stop == "stop" && path.starts_with('/') => true,
        _ => false,
    }
}
