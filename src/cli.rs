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

pub const HELP: &str = "Session Keeper\nUsage: session-keeper-rust [options]\n\nOptions:\n  (none)                       Open the window and run the watchdog while it is open\n  --check-now                  Run one watchdog tick in the terminal and exit\n  --status                     Print session statuses and exit\n  --kick <session-id>          Send the nudge prompt to one session now\n  --models [filter]            List available models, optionally filtered\n  --set-model <sid> <id> <provider>   Switch one session's model\n  --export-audit               Write history + log to ~/session-keeper-audit.txt
  --dry-run                    Never send real nudges\n  --help                       Show this help\n";
