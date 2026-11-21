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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub dry_run: bool,
}

pub fn parse_args(args: &[String]) -> Options {
    let mut mode = Mode::Gui;
    let mut dry_run = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--check-now" => mode = Mode::CheckNow,
            "--status" => mode = Mode::Status,
            "--export-audit" => mode = Mode::ExportAudit,
            "--watch" => mode = Mode::Watch,
            "--version" => mode = Mode::Version,
            "--export-state" => {
                if i + 1 < args.len() {
                    mode = Mode::ExportState(args[i + 1].clone());
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
            "--dry-run" => dry_run = true,
            "--help" | "-h" => {
                mode = Mode::Help;
                break;
            }
            "--kick" => {
                if i + 1 < args.len() {
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
                if i + 3 < args.len() {
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
    Options { mode, dry_run }
}

pub const HELP: &str = "Session Keeper\nUsage: session-keeper-rust [options]\n\nOptions:\n  (none)                       Open the window and run the watchdog while it is open\n  --check-now                  Run one watchdog tick in the terminal and exit\n  --status                     Print session statuses and exit\n  --kick <session-id>          Send the nudge prompt to one session now\n  --models [filter]            List available models, optionally filtered\n  --set-model <sid> <id> <provider>   Switch one session's model\n  --pause <sid>                Pause one session
  --resume <sid>               Resume one session
  --set-prompt <sid> <text>    Set that session's nudge prompt
  --watch                      Print status every tick in the terminal
  --version                    Show version
  --export-state <path>        Copy state.json to a backup path
  --export-audit               Write history + log to ~/session-keeper-audit.txt
  --dry-run                    Never send real nudges\n  --help                       Show this help\n";
