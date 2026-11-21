#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Gui,
    CheckNow,
    Status,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub dry_run: bool,
}

pub fn parse_args(args: &[String]) -> Options {
    let mut mode = Mode::Gui;
    let mut dry_run = false;
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--check-now" => mode = Mode::CheckNow,
            "--status" => mode = Mode::Status,
            "--dry-run" => dry_run = true,
            "--help" | "-h" => mode = Mode::Help,
            _ => mode = Mode::Help,
        }
    }
    Options { mode, dry_run }
}

pub const HELP: &str = "Session Keeper\nUsage: session-keeper-rust [options]\n\nOptions:\n  (none)      Open the window and run the watchdog while it is open\n  --check-now Run one watchdog tick in the terminal and exit\n  --status    Print session statuses and exit\n  --dry-run   Never send real nudges (works with --check-now)\n  --help      Show this help\n";
