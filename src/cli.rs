#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Gui,
    CheckNow,
    Status,
    Help,
    Kick(String),
    Models(Option<String>),
    SetModel(String, String, String),
    ExportAudit,
    Pause(String),
    Resume(String),
    SetPrompt(String, String),
    Watch,
    Version,
    ExportState(String),
    ImportState(String),
    SetSetting(String, String),
    Sessions,
    Doctor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub dry_run: bool,
    pub json: bool,
    pub explicit_dry: bool,
}

// Session ids and paths must be real values, not empty or another flag.
fn valid_value(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('-')
}

pub fn parse_args(args: &[String]) -> Options {
    let mut mode = Mode::Gui;
    let mut dry_run = false;
    let mut json = false;
    let mut explicit_dry = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--check-now" => mode = Mode::CheckNow,
            "--status" => mode = Mode::Status,
            "--export-audit" => mode = Mode::ExportAudit,
            "--sessions" => mode = Mode::Sessions,
            "--set-setting" => {
                if i + 2 < args.len() && !args[i + 1].starts_with('-') {
                    mode = Mode::SetSetting(args[i + 1].clone(), args[i + 2].clone());
                    i += 2;
                } else {
                    mode = Mode::Help;
                    break;
                }
            }
            "--watch" => mode = Mode::Watch,
            "--version" => mode = Mode::Version,
            "--doctor" => mode = Mode::Doctor,
            "--export-state" | "--import-state" => {
                let flag = args[i].clone();
                if i + 1 < args.len() && valid_value(&args[i + 1]) {
                    mode = if flag == "--export-state" {
                        Mode::ExportState(args[i + 1].clone())
                    } else {
                        Mode::ImportState(args[i + 1].clone())
                    };
                    i += 1;
                } else {
                    mode = Mode::Help;
                    break;
                }
            }
            "--pause" | "--resume" | "--set-prompt" => {
                let flag = args[i].clone();
                let need = if flag == "--set-prompt" { 2 } else { 1 };
                if i + need >= args.len() {
                    mode = Mode::Help;
                    break;
                }
                mode = if flag == "--pause" {
                    Mode::Pause(args[i + 1].clone())
                } else if flag == "--resume" {
                    Mode::Resume(args[i + 1].clone())
                } else {
                    Mode::SetPrompt(args[i + 1].clone(), args[i + 2].clone())
                };
                i += need;
            }
            "--dry-run" => { dry_run = true; explicit_dry = true; },
            "--json" => json = true,
            "--help" | "-h" => {
                mode = Mode::Help;
                break;
            }
            "--kick" => {
                if i + 1 < args.len() && valid_value(&args[i + 1]) {
                    mode = Mode::Kick(args[i + 1].clone());
                    i += 1;
                } else {
                    mode = Mode::Help;
                    break;
                }
            }
            "--models" => {
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    mode = Mode::Models(Some(args[i + 1].clone()));
                    i += 1;
                } else {
                    mode = Mode::Models(None);
                }
            }
            "--set-model" => {
                if i + 3 < args.len() && valid_value(&args[i + 1]) {
                    mode = Mode::SetModel(
                        args[i + 1].clone(),
                        args[i + 2].clone(),
                        args[i + 3].clone(),
                    );
                    i += 3;
                } else {
                    mode = Mode::Help;
                    break;
                }
            }
            _ => {
                mode = Mode::Help;
                break;
            }
        }
        i += 1;
    }
    Options { mode, dry_run, json, explicit_dry }
}

pub const HELP: &str = "Session Keeper
Usage: session-keeper-rust [options]

Options:
  (none)                            Open the window and run the watchdog while it is open
  --check-now                       Run one watchdog tick in the terminal and exit
  --status                          Print session statuses and exit
  --sessions                        List live OpenCode sessions
  --kick <session-id>               Send the nudge prompt to one session now
  --models [filter]                 List available models, optionally filtered
  --set-model <sid> <id> <provider> Switch one session's model
  --pause <sid>                     Pause one session
  --resume <sid>                    Resume one session
  --set-prompt <sid> <text>         Set that session's nudge prompt
  --set-setting <key> <value>       Change min_gap, cap, global_cap, interval, cooldown, away, enabled, dry_run, notify
  --watch                           Print status every tick in the terminal
  --version                         Show version
  --doctor                          Check database, credentials, API and notifier
  --export-state <path>             Copy state.json to a backup path
  --import-state <path>             Restore state.json from a backup
  --export-audit                    Write history + log to ~/session-keeper-audit.txt
  --json                            Machine-readable output for --status / --check-now
  --dry-run                         Never send real nudges
  --help                            Show this help
";
