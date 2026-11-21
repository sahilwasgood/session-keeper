use glib::clone;
use gtk::prelude::*;
use rusqlite::{Connection, OpenFlags};
use session_keeper_rust::cli::{parse_args, Mode, HELP};
use session_keeper_rust::config::Config;
use session_keeper_rust::db::{classify, last_assistant_text, last_assistant_ts, last_idle_ts, session_exists, session_model, session_title, Status};
use session_keeper_rust::nudge::{append_history, decide, fetch_models, log_line, notify, nudges_in_last_hour_all, send_nudge, switch_model, Decision, ModelInfo};
use session_keeper_rust::process::{pid_alive, read_lock};
use session_keeper_rust::state::{Session, State};
use session_keeper_rust::safety::{fingerprint, should_nudge_after_debounce};
use session_keeper_rust::ui::{format_age, matches_filter, needs_attention, prompt_display, short_id, status_changed, status_class, status_priority, status_text, summary_line, CSS};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static MODEL_TX: OnceLock<Sender<(String, Vec<ModelInfo>)>> = OnceLock::new();
static KICK_TX: OnceLock<Sender<(String, Result<(), String>)>> = OnceLock::new();

#[derive(Clone)]
struct SessionView {
    status: Status,
    title: String,
    progress_ms: i64,
    idle_ms: i64,
    reason: String,
    exists: bool,
    model: String,
}

struct TickOutcome {
    summary: String,
    state: State,
    views: HashMap<String, SessionView>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn open_db() -> Option<Connection> {
    let drive = Config::drive();
    if !drive.exists() || !Config::db_path().exists() {
        return None;
    }
    Connection::open_with_flags(Config::db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .and_then(|c| {
            c.busy_timeout(Duration::from_millis(1200)).ok()?;
            // A locked or unreadable DB must be reported as unavailable, not
            // silently read as "no events" for every session.
            c.query_row("SELECT 1 FROM session_message LIMIT 1", [], |_| Ok(())).ok()?;
            Some(c)
        })
}

fn icon_button(label: &str, icon: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.set_image(Some(&gtk::Image::from_icon_name(Some(icon), gtk::IconSize::Button)));
    b.set_always_show_image(true);
    b
}

fn reason(status: &Status) -> &'static str {
    match status {
        Status::Due => "idle",
        Status::Nudged => "already_nudged",
        Status::Waiting => "waiting_on_question",
        Status::Stalled => "no_new_output",
        Status::Clean => "working",
    }
}

fn tick(state: &Rc<RefCell<State>>) -> String {
    let now = now_ms();
    let conn = match open_db() {
        Some(c) => c,
        None => {
            let msg = "Watchdog paused: drive or database unavailable".to_string();
            log_line(&msg);
            return msg;
        }
    };
    let sessions: Vec<Session> = state.borrow().sessions.clone();
    let global_cap = state.borrow().global_nudge_cap;
    let mut used_global = if global_cap > 0 { nudges_in_last_hour_all(now) } else { 0 };
    let mut nudged = 0;
    let mut held = 0;

    for s in &sessions {
        let (status, decision) = {
            let st = state.borrow();
            let status = classify(&conn, &s.id, &st);
            let decision = decide(&st, s, &status, now);
            (status, decision)
        };

        let status_name = format!("{:?}", status);
        if status == Status::Waiting {
            let prev = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.last_status.clone()).unwrap_or_default();
            let last_alert = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.last_alert_ms).unwrap_or(0);
            if status_changed(&prev, &status_name) && now - last_alert > 300_000 {
                let title = session_title(&conn, &s.id);
                notify("Session Keeper", &format!("{} asked a question", if title.is_empty() { &s.id } else { &title }));
                if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
                    x.last_alert_ms = now;
                }
                log_line(&format!("{}: alerted waiting on question", s.id));
            }
        }
        if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
            x.last_status = status_name;
        }

        if decision != Decision::Nudge {
            if status != Status::Due {
                if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
                    x.idle_streak = 0;
                }
            }
            if decision == Decision::Hold {
                log_line(&format!("{}: hold {}", s.id, reason(&status)));
                held += 1;
            }
            continue;
        }

        let fp = fingerprint(&last_assistant_text(&conn, &s.id));
        let hash_same = state
            .borrow()
            .sessions
            .iter()
            .find(|x| x.id == s.id)
            .and_then(|x| x.last_hash.as_deref())
            == Some(fp.as_str());
        if hash_same {
            log_line(&format!("{}: hold loop (unchanged output)", s.id));
            held += 1;
            continue;
        }

        let streak = {
            let mut st = state.borrow_mut();
            let x = st.sessions.iter_mut().find(|x| x.id == s.id).unwrap();
            x.idle_streak += 1;
            x.idle_streak
        };
        if !should_nudge_after_debounce(streak) {
            log_line(&format!("{}: hold debounce {}/2", s.id, streak));
            held += 1;
            continue;
        }

        if global_cap > 0 && used_global >= global_cap {
            log_line(&format!("{}: hold global cap {}/{}", s.id, used_global, global_cap));
            held += 1;
            continue;
        }

        if Config::dry_run() {
            log_line(&format!("{}: would nudge (dry run)", s.id));
            held += 1;
            continue;
        }

        let raw = s.prompt.clone().unwrap_or_else(|| state.borrow().default_prompt.clone());
        let prompt = raw.trim().to_string();
        let prompt = if prompt.is_empty() { "continue".to_string() } else { prompt };
        match send_nudge(&Config::api_base(), &Config::password(), &s.id, &prompt) {
            Ok(()) => {
                let ts = last_idle_ts(&conn, &s.id);
                let title = session_title(&conn, &s.id);
                {
                    let mut st = state.borrow_mut();
                    if let Some(x) = st.sessions.iter_mut().find(|x| x.id == s.id) {
                        x.handled = ts;
                        x.prompted = now;
                        x.last_nudge = now;
                        x.nudges += 1;
                        x.last_action = format!("nudged {}", session_keeper_rust::config::format_log_time(now / 1000));
                        x.idle_streak = 0;
                        x.last_hash = Some(fp);
                    }
                }
                append_history(&s.id, "nudge", &prompt);
                notify("Session Keeper", &format!("Nudged {}", if title.is_empty() { &s.id } else { &title }));
                log_line(&format!("{}: prompted", s.id));
                nudged += 1;
                used_global += 1;
            }
            Err(e) => {
                let mut st = state.borrow_mut();
                if let Some(x) = st.sessions.iter_mut().find(|x| x.id == s.id) {
                    x.last_action = format!("failed {}", e);
                }
                log_line(&format!("{}: prompt failed {}", s.id, e));
            }
        }
    }

    state.borrow_mut().last_tick_ms = now;
    let _ = state.borrow().save();
    summary_line(&session_keeper_rust::config::format_log_time(now / 1000), nudged, held)
}

fn run_tick(snapshot: State) -> TickOutcome {
    // Runs on a worker thread. No GTK calls here.
    let st = Rc::new(RefCell::new(snapshot));
    let summary = tick(&st);
    let mut views = HashMap::new();
    if let Some(conn) = open_db() {
        for s in st.borrow().sessions.iter() {
            let status = classify(&conn, &s.id, &st.borrow());
            let view = SessionView {
                status,
                title: session_title(&conn, &s.id),
                progress_ms: last_assistant_ts(&conn, &s.id),
                idle_ms: last_idle_ts(&conn, &s.id),
                reason: if session_exists(&conn, &s.id) { reason(&status).to_string() } else { "missing_from_opencode".to_string() },
                exists: session_exists(&conn, &s.id),
                model: session_model(&conn, &s.id),
            };
            views.insert(s.id.clone(), view);
        }
    }
    let final_state = st.borrow().clone();
    TickOutcome { summary, state: final_state, views }
}

fn launch_tick(
    state: &Rc<RefCell<State>>,
    tx: &Sender<TickOutcome>,
    busy: &Rc<Cell<bool>>,
) {
    if busy.get() {
        return;
    }
    busy.set(true);
    let snapshot = state.borrow().clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let outcome = run_tick(snapshot);
        let _ = tx.send(outcome);
    });
}

fn today_digest() -> String {
    let now = now_ms();
    let day_start = now - (now % 86_400_000);
    let nudges = std::fs::read_to_string(Config::history_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v.get("kind").and_then(|s| s.as_str()) == Some("nudge"))
        .filter(|v| v.get("t").and_then(|t| t.as_i64()).unwrap_or(0) >= day_start)
        .count();
    format!("Today: {} nudges sent", nudges)
}

fn tail_file(path: &Path, lines: usize) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

fn refresh(
    state: &Rc<RefCell<State>>,
    list: &gtk::Box,
    views: &Rc<RefCell<HashMap<String, SessionView>>>,
    filter: &Rc<RefCell<String>>,
    attention: &Rc<RefCell<bool>>,
) {
    for child in list.children() {
        list.remove(&child);
    }
    let sessions = state.borrow().sessions.clone();
    let default_prompt = state.borrow().default_prompt.clone();
    let view_map = views.borrow();
    let filter_text = filter.borrow();
    let attention_on = *attention.borrow();
    let mut items: Vec<_> = sessions.iter().map(|s| (s, view_map.get(&s.id))).collect();
    if attention_on {
        items.retain(|(_, v)| v.map(|x| needs_attention(&x.status)).unwrap_or(false));
    }
    items.sort_by_key(|(s, v)| {
        let prio = v.map(|x| status_priority(&x.status)).unwrap_or(4);
        let name = format!("{}{}", v.map(|x| x.title.as_str()).unwrap_or(""), s.id);
        (prio, name)
    });
    for (s, view) in items {
        let view = view;
        let status = view.map(|v| v.status).unwrap_or(Status::Clean);
        let title = view.map(|v| v.title.as_str()).unwrap_or("");
        if !matches_filter(&s.id, title, &filter_text) { continue; }

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.style_context().add_class("card");

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let badge = gtk::Label::new(Some(status_text(&status)));
        badge.style_context().add_class("badge");
        badge.style_context().add_class(status_class(&status));
        badge.set_size_request(118, -1);
        if let Some(v) = view { badge.set_tooltip_text(Some(&v.reason)); }
        if view.map(|v| !v.exists).unwrap_or(false) {
            badge.set_label("missing");
            badge.style_context().add_class("badge-stalled");
        }
        top.add(&badge);

        let name = if title.is_empty() {
            short_id(&s.id)
        } else {
            format!("{}   {}", title, short_id(&s.id))
        };
        let id_label = gtk::Label::new(Some(&name));
        id_label.style_context().add_class("session-id");
        id_label.set_xalign(0.0);
        id_label.set_hexpand(true);
        id_label.set_tooltip_text(Some(&s.id));
        top.add(&id_label);

        let pause = gtk::Button::with_label(if s.enabled { "Pause" } else { "Resume" });
        let st = state.clone();
        let list_c = list.clone();
        let views_c = views.clone();
        let filter_c = filter.clone();
        let attention_c = attention.clone();
        let sid = s.id.clone();
        pause.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid => move |_| {
            if let Some(x) = st.borrow_mut().sessions.iter_mut().find(|x| x.id == sid) {
                x.enabled = !x.enabled;
            }
            let _ = st.borrow().save();
            refresh(&st, &list_c, &views_c, &filter_c, &attention_c);
        }));
        top.add(&pause);
        card.add(&top);

        let meta = gtk::Label::new(Some(&format!(
            "prompt: {}   nudges {}   last: {}",
            prompt_display(s.prompt.as_deref(), &default_prompt),
            s.nudges,
            if s.last_action.is_empty() { "never" } else { &s.last_action }
        )));
        let receipts = view.map(|v| {
            format!(
                "state {} | last progress {} | idle {}",
                v.reason,
                format_age(v.progress_ms, now_ms()),
                if v.idle_ms > 0 { format_age(v.idle_ms, now_ms()) } else { "none".to_string() }
            )
        }).unwrap_or_default();
        let receipts = match view {
            Some(v) if !v.model.is_empty() => format!("{} | model {}", receipts, v.model),
            _ => receipts,
        };
        let receipt_label = gtk::Label::new(Some(&receipts));
        receipt_label.style_context().add_class("note");
        receipt_label.set_xalign(0.0);
        card.add(&receipt_label);
        meta.style_context().add_class("session-meta");
        meta.set_xalign(0.0);
        meta.set_line_wrap(true);
        card.add(&meta);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let edit = icon_button("Edit prompt", "document-edit-symbolic");
        let st = state.clone();
        let list_c = list.clone();
        let views_c = views.clone();
        let filter_c = filter.clone();
        let attention_c = attention.clone();
        let sid_edit = s.id.clone();
        let cur = s.prompt.clone();
        edit.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid_edit, @strong cur => move |_| {
            edit_prompt(&st, &list_c, &views_c, &filter_c, &attention_c, &sid_edit, cur.as_deref());
        }));
        actions.add(&edit);

        let model_btn = icon_button("Model", "preferences-system-symbolic");
        let sid_model = s.id.clone();
        let tx_model = MODEL_TX.get().expect("model channel not initialized").clone();
        model_btn.connect_clicked(move |_| {
            let sid = sid_model.clone();
            let tx = tx_model.clone();
            let api = Config::api_base();
            let pw = Config::password();
            std::thread::spawn(move || {
                let models = fetch_models(&api, &pw);
                let _ = tx.send((sid, models));
            });
        });
        actions.add(&model_btn);

        let kick = icon_button("Kick", "mail-send-symbolic");
        let sid_kick = s.id.clone();
        let prompt_kick = s.prompt.clone().unwrap_or_default();
        let default_kick = default_prompt.clone();
        kick.connect_clicked(move |_| {
            let tx = match KICK_TX.get() { Some(t) => t.clone(), None => return };
            let sid = sid_kick.clone();
            let prompt = if prompt_kick.trim().is_empty() { default_kick.clone() } else { prompt_kick.clone() };
            let api = Config::api_base();
            let pw = Config::password();
            std::thread::spawn(move || {
                let result = if Config::dry_run() {
                    Ok(())
                } else {
                    send_nudge(&api, &pw, &sid, &prompt)
                };
                if result.is_ok() && !Config::dry_run() {
                    append_history(&sid, "nudge", &prompt);
                    log_line(&format!("{}: kicked from UI", sid));
                }
                let _ = tx.send((sid, result));
            });
        });
        actions.add(&kick);

        let del = icon_button("Remove", "user-trash-symbolic");
        let st = state.clone();
        let list_c = list.clone();
        let views_c = views.clone();
        let filter_c = filter.clone();
        let attention_c = attention.clone();
        let sid_del = s.id.clone();
        del.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid_del => move |_| {
            {
                let mut st_mut = st.borrow_mut();
                st_mut.sessions.retain(|x| x.id != sid_del);
                let _ = st_mut.save();
            }
            refresh(&st, &list_c, &views_c, &filter_c, &attention_c);
        }));
        actions.add(&del);
        card.add(&actions);

        list.add(&card);
    }
    if sessions.is_empty() {
        let empty = gtk::Label::new(Some("No watched sessions. Add a session id below."));
        empty.style_context().add_class("note");
        list.add(&empty);
    }
    list.show_all();
}

fn edit_prompt(
    state: &Rc<RefCell<State>>,
    list: &gtk::Box,
    views: &Rc<RefCell<HashMap<String, SessionView>>>,
    filter: &Rc<RefCell<String>>,
    attention: &Rc<RefCell<bool>>,
    sid: &str,
    current: Option<&str>,
) {
    let dialog = gtk::Dialog::with_buttons(
        Some("Edit nudge prompt"),
        None::<&gtk::Window>,
        gtk::DialogFlags::MODAL,
        &[("Cancel", gtk::ResponseType::Cancel), ("Save", gtk::ResponseType::Ok)],
    );
    let entry = gtk::Entry::new();
    entry.set_text(current.unwrap_or(""));
    entry.set_hexpand(true);
    dialog.content_area().add(&entry);
    dialog.show_all();

    let st = state.clone();
    let list_c = list.clone();
    let views_c = views.clone();
    let filter_c = filter.clone();
    let attention_c = attention.clone();
    let sid = sid.to_string();
    dialog.connect_response(move |dlg, resp| {
        if resp == gtk::ResponseType::Ok {
            let text = entry.text().trim().to_string();
            {
                let mut st_mut = st.borrow_mut();
                if let Some(s) = st_mut.sessions.iter_mut().find(|s| s.id == sid) {
                    s.prompt = if text.is_empty() { None } else { Some(text) };
                }
                let _ = st_mut.save();
            }
            refresh(&st, &list_c, &views_c, &filter_c, &attention_c);
        }
        dlg.close();
    });
}

fn arm_timer(
    state: &Rc<RefCell<State>>,
    tx: &Sender<TickOutcome>,
    busy: &Rc<Cell<bool>>,
    timer: &Rc<RefCell<Option<glib::SourceId>>>,
) {
    if let Some(id) = timer.borrow_mut().take() {
        id.remove();
    }
    let interval = state.borrow().tick_interval_s.clamp(5, 300) as u32;
    let st = state.clone();
    let tx = tx.clone();
    let busy = busy.clone();
    let id = glib::timeout_add_seconds_local(interval, move || {
        launch_tick(&st, &tx, &busy);
        glib::ControlFlow::Continue
    });
    *timer.borrow_mut() = Some(id);
}

struct LockGuard(std::path::PathBuf);
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let opts = parse_args(&raw);
    if opts.dry_run {
        std::env::set_var("SESSION_KEEPER_DRY_RUN", "1");
    }
    match opts.mode {
        Mode::Help => {
            print!("{}", HELP);
            return;
        }
        Mode::CheckNow => {
            let st = Rc::new(RefCell::new(State::load()));
            let out = run_tick(st.borrow().clone());
            println!("{}", out.summary);
            return;
        }
        Mode::Status => {
            let st = State::load();
            let conn = open_db();
            for s in &st.sessions {
                let status = conn.as_ref().map(|c| classify(c, &s.id, &st)).unwrap_or(Status::Clean);
                println!("{}  {}  {}  nudges={}  last={}", short_id(&s.id), s.id,
                    status_text(&status), s.nudges, s.last_action);
            }
            return;
        }
        Mode::Kick(sid) => {
            let st = State::load();
            let session = match st.sessions.iter().find(|s| s.id == sid) {
                Some(s) => s.clone(),
                None => {
                    eprintln!("Session {} is not in the watch list", sid);
                    return;
                }
            };
            let prompt = session.prompt.clone().unwrap_or_else(|| st.default_prompt.clone());
            let prompt = prompt.trim();
            let prompt = if prompt.is_empty() { "continue" } else { prompt };
            if Config::dry_run() {
                println!("Would kick {} with: {}", sid, prompt);
                return;
            }
            match send_nudge(&Config::api_base(), &Config::password(), &sid, prompt) {
                Ok(()) => {
                    append_history(&sid, "nudge", prompt);
                    log_line(&format!("{}: kicked from CLI", sid));
                    println!("Kicked {}", sid);
                }
                Err(e) => {
                    eprintln!("Kick failed: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::Models(filter) => {
            let models = fetch_models(&Config::api_base(), &Config::password());
            let mut shown = 0;
            for m in models.iter().filter(|m| match &filter {
                Some(f) => m.id.contains(f) || m.name.contains(f),
                None => true,
            }) {
                println!("{} | {} | {}", m.id, m.provider, m.name);
                shown += 1;
                if shown >= 30 { break; }
            }
            println!("Total available: {} models (showing {})", models.len(), shown);
            return;
        }
        Mode::SetModel(sid, id, provider) => {
            match switch_model(&Config::api_base(), &Config::password(), &sid, &id, &provider) {
                Ok(()) => {
                    log_line(&format!("{}: model switched to {} ({})", sid, id, provider));
                    println!("Model switched for {}", sid);
                }
                Err(e) => {
                    eprintln!("Model switch failed: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::Gui => {}
    }

    let lock_path = Config::lock_path();
    if let Some(pid) = read_lock(&lock_path) {
        if pid_alive(pid) && pid != std::process::id() as i32 {
            eprintln!("Session Keeper is already running (pid {}).", pid);
            return;
        }
    }
    if std::fs::write(&lock_path, std::process::id().to_string()).is_err() {
        eprintln!("Could not write lock file; continuing without single-instance protection.");
    }
    let _lock_guard = LockGuard(lock_path);

    gtk::init().expect("GTK3 failed to initialize");

    if let Some(screen) = gtk::gdk::Screen::default() {
        let provider = gtk::CssProvider::new();
        let _ = provider.load_from_data(CSS.as_bytes());
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let state = Rc::new(RefCell::new(State::load()));
    if !opts.dry_run {
        let st = state.borrow();
        std::env::set_var("SESSION_KEEPER_DRY_RUN", if st.persist_dry_run { "1" } else { "0" });
        std::env::set_var("SESSION_KEEPER_NOTIFY", if st.persist_notifications { "1" } else { "0" });
    }
    let views: Rc<RefCell<HashMap<String, SessionView>>> = Rc::new(RefCell::new(HashMap::new()));
    let filter: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let attention: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let busy = Rc::new(Cell::new(false));
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let (tx, rx) = channel::<TickOutcome>();
    let (model_tx, model_rx) = channel::<(String, Vec<ModelInfo>)>();
    let _ = MODEL_TX.set(model_tx.clone());
    let (switch_tx, switch_rx) = channel::<(String, Result<(), String>)>();
    let (kick_tx, kick_rx) = channel::<(String, Result<(), String>)>();
    let _ = KICK_TX.set(kick_tx.clone());
    let model_cache: Rc<RefCell<HashMap<String, Vec<ModelInfo>>>> = Rc::new(RefCell::new(HashMap::new()));

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Session Keeper");
    window.set_default_size(960, 680);
    window.set_size_request(760, 520);
    window.connect_destroy(|_| gtk::main_quit());
    let st_key = state.clone();
    let tx_key = tx.clone();
    let busy_key = busy.clone();
    window.connect_key_press_event(move |_, key| {
        use gtk::gdk::keys::constants;
        if key.keyval() == constants::F5 {
            launch_tick(&st_key, &tx_key, &busy_key);
            return glib::Propagation::Stop;
        }
        if key.keyval() == constants::q
            && key.state().contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            gtk::main_quit();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    let header = gtk::HeaderBar::new();
    header.set_title(Some("Session Keeper"));
    header.set_subtitle(Some("On-demand OpenCode watchdog"));
    header.set_show_close_button(true);
    window.set_titlebar(Some(&header));
    let quit = icon_button("Quit", "window-close-symbolic");
    quit.style_context().add_class("destructive-action");
    quit.connect_clicked(|_| gtk::main_quit());
    header.pack_end(&quit);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    outer.set_border_width(12);
    window.add(&outer);

    let summary = gtk::Label::new(Some("Starting..."));
    summary.style_context().add_class("summary");
    summary.set_xalign(0.0);
    outer.add(&summary);

    let note = gtk::Label::new(Some("Watchdog runs only while this window is open. Closing it stops everything."));
    note.style_context().add_class("note");
    note.set_xalign(0.0);
    outer.add(&note);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("Session id (ses_...)"));
    entry.set_hexpand(true);
    controls.add(&entry);

    let filter_entry = gtk::Entry::new();
    filter_entry.set_placeholder_text(Some("Filter sessions..."));
    filter_entry.set_width_chars(18);
    controls.add(&filter_entry);
    let attention_cb = gtk::CheckButton::with_label("Attention only");
    controls.add(&attention_cb);

    let add = icon_button("Add session", "list-add-symbolic");
    let st_add = state.clone();
    let list_add = Rc::new(RefCell::new(gtk::Box::new(gtk::Orientation::Vertical, 2)));
    let views_add = views.clone();
    let filter_add = filter.clone();
    let attention_add = attention.clone();
    let summary_add = summary.clone();
    let entry_add = entry.clone();
    add.connect_clicked(clone!(@strong st_add, @strong list_add, @strong views_add, @strong filter_add, @strong summary_add, @strong entry_add => move |_| {
        let sid = entry_add.text().trim().to_string();
        if sid.starts_with("ses_") && !st_add.borrow().sessions.iter().any(|s| s.id == sid) {
            st_add.borrow_mut().sessions.push(Session {
                id: sid, enabled: true, prompt: None, handled: 0, prompted: 0,
                last_nudge: 0, nudges: 0, last_action: "added".to_string(), idle_streak: 0, last_hash: None, last_status: String::new(), last_alert_ms: 0,
            });
            let _ = st_add.borrow().save();
            entry_add.set_text("");
            summary_add.set_label("Session added");
            refresh(&st_add, &list_add.borrow(), &views_add, &filter_add, &attention_add);
        }
    }));
    controls.add(&add);

    let check = icon_button("Check now", "view-refresh-symbolic");
    check.style_context().add_class("suggested-action");
    let st_check = state.clone();
    let tx_check = tx.clone();
    let busy_check = busy.clone();
    let summary_check = summary.clone();
    check.connect_clicked(move |_| {
        summary_check.set_label("Checking...");
        launch_tick(&st_check, &tx_check, &busy_check);
    });
    controls.add(&check);
    outer.add(&controls);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.add(&list);
    outer.add(&scroll);

    let attention_toggle = attention.clone();
    let st_att = state.clone();
    let list_att = list.clone();
    let views_att = views.clone();
    let filter_att = filter.clone();
    attention_cb.connect_toggled(move |b| {
        *attention_toggle.borrow_mut() = b.is_active();
        refresh(&st_att, &list_att, &views_att, &filter_att, &attention_toggle);
    });

    let filter_state = filter.clone();
    let attention_filter = attention.clone();
    let list_filter = list.clone();
    let views_filter = views.clone();
    let st_filter = state.clone();
    filter_entry.connect_changed(move |e| {
        *filter_state.borrow_mut() = e.text().to_string();
        refresh(&st_filter, &list_filter, &views_filter, &filter_state, &attention_filter);
    });

    // Settings panel from the old web dashboard.
    let settings = gtk::Expander::new(Some("Settings: prompt, gap, hourly cap, away mode"));
    let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    settings_box.set_border_width(8);

    let def_prompt = gtk::Entry::new();
    def_prompt.set_text(&state.borrow().default_prompt);
    def_prompt.set_placeholder_text(Some("Default nudge prompt"));
    let min_gap = gtk::SpinButton::new(Some(&gtk::Adjustment::new(state.borrow().min_gap as f64, 0.0, 3600.0, 1.0, 10.0, 0.0)), 1.0, 0);
    min_gap.set_value(state.borrow().min_gap as f64);
    let cap = gtk::SpinButton::new(Some(&gtk::Adjustment::new(state.borrow().max_nudges_per_hour as f64, 0.0, 500.0, 1.0, 10.0, 0.0)), 1.0, 0);
    cap.set_value(state.borrow().max_nudges_per_hour as f64);
    let away = gtk::CheckButton::with_label("Away mode (never nudge a session that asked a question)");
    away.set_active(state.borrow().away_mode);

    let row1 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row1.add(&gtk::Label::new(Some("Default prompt:")));
    def_prompt.set_hexpand(true);
    row1.add(&def_prompt);
    settings_box.add(&row1);

    let row2 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row2.add(&gtk::Label::new(Some("Min gap (s):")));
    row2.add(&min_gap);
    row2.add(&gtk::Label::new(Some("Max nudges/hour (0 = unlimited):")));
    row2.add(&cap);
    settings_box.add(&row2);
    settings_box.add(&away);

    let global_cap = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(state.borrow().global_nudge_cap as f64, 0.0, 500.0, 1.0, 10.0, 0.0)),
        1.0, 0,
    );
    let row3 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row3.add(&gtk::Label::new(Some("Global nudge cap/hour:")));
    row3.add(&global_cap);
    row3.add(&gtk::Label::new(Some("Tick interval (s):")));
    let interval_spin = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(state.borrow().tick_interval_s as f64, 5.0, 300.0, 5.0, 30.0, 0.0)),
        1.0, 0,
    );
    row3.add(&interval_spin);
    settings_box.add(&row3);

    let enabled_cb = gtk::CheckButton::with_label("Watching enabled (global off switch)");
    enabled_cb.set_active(state.borrow().enabled);
    settings_box.add(&enabled_cb);
    let dry_cb = gtk::CheckButton::with_label("Dry run: preview decisions, send no nudges");
    dry_cb.set_active(Config::dry_run());
    settings_box.add(&dry_cb);
    let notify_cb = gtk::CheckButton::with_label("Desktop notifications on nudge");
    notify_cb.set_active(Config::notifications());
    settings_box.add(&notify_cb);

    let save_settings = icon_button("Save settings", "document-save-symbolic");
    save_settings.style_context().add_class("suggested-action");
    let st_settings = state.clone();
    let tx_settings = tx.clone();
    let busy_settings = busy.clone();
    let timer_settings = timer.clone();
    save_settings.connect_clicked(move |_| {
        let mut st = st_settings.borrow_mut();
        let p = def_prompt.text().trim().to_string();
        st.default_prompt = if p.is_empty() { "continue, dont stop".to_string() } else { p };
        st.min_gap = min_gap.value() as i64;
        st.max_nudges_per_hour = cap.value() as i64;
        st.away_mode = away.is_active();
        st.global_nudge_cap = global_cap.value() as i64;
        st.enabled = enabled_cb.is_active();
        let _ = st.save();
        st.persist_dry_run = dry_cb.is_active();
        st.persist_notifications = notify_cb.is_active();
        std::env::set_var("SESSION_KEEPER_DRY_RUN", if st.persist_dry_run { "1" } else { "0" });
        std::env::set_var("SESSION_KEEPER_NOTIFY", if st.persist_notifications { "1" } else { "0" });
        arm_timer(&st_settings, &tx_settings, &busy_settings, &timer_settings);
    });
    settings_box.add(&save_settings);

    let bulk_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let pause_all = icon_button("Pause all", "media-playback-pause-symbolic");
    let st_pause = state.clone();
    let list_pause = list.clone();
    let views_pause = views.clone();
    let filter_pause = filter.clone();
    let attention_pause = attention.clone();
    pause_all.connect_clicked(move |_| {
        for s in st_pause.borrow_mut().sessions.iter_mut() { s.enabled = false; }
        let _ = st_pause.borrow().save();
        refresh(&st_pause, &list_pause, &views_pause, &filter_pause, &attention_pause);
    });
    bulk_row.add(&pause_all);
    let resume_all = icon_button("Resume all", "media-playback-start-symbolic");
    let st_resume = state.clone();
    let list_resume = list.clone();
    let views_resume = views.clone();
    let filter_resume = filter.clone();
    let attention_resume = attention.clone();
    resume_all.connect_clicked(move |_| {
        for s in st_resume.borrow_mut().sessions.iter_mut() { s.enabled = true; }
        let _ = st_resume.borrow().save();
        refresh(&st_resume, &list_resume, &views_resume, &filter_resume, &attention_resume);
    });
    bulk_row.add(&resume_all);
    settings_box.add(&bulk_row);

    let io_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let export_btn = icon_button("Export settings", "document-save-as-symbolic");
    let st_export = state.clone();
    let summary_io = summary.clone();
    export_btn.connect_clicked(move |_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/mint".to_string());
        let path = std::path::Path::new(&home).join("session-keeper-export.json");
        let text = serde_json::to_string_pretty(&*st_export.borrow()).unwrap_or_default();
        match std::fs::write(&path, text) {
            Ok(()) => summary_io.set_label(&format!("Exported to {}", path.display())),
            Err(e) => summary_io.set_label(&format!("Export failed: {}", e)),
        }
    });
    io_row.add(&export_btn);
    let import_btn = icon_button("Import settings", "document-open-symbolic");
    let st_import = state.clone();
    let list_import = list.clone();
    let views_import = views.clone();
    let filter_import = filter.clone();
    let attention_import = attention.clone();
    let summary_import = summary.clone();
    import_btn.connect_clicked(move |_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/mint".to_string());
        let path = std::path::Path::new(&home).join("session-keeper-export.json");
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<State>(&s).ok())
        {
            Some(new_state) => {
                *st_import.borrow_mut() = new_state;
                let _ = st_import.borrow().save();
                summary_import.set_label(&format!("Imported from {}", path.display()));
                refresh(&st_import, &list_import, &views_import, &filter_import, &attention_import);
            }
            None => summary_import.set_label("Import failed: no valid export file"),
        }
    });
    io_row.add(&import_btn);
    settings_box.add(&io_row);
    settings.add(&settings_box);
    outer.add(&settings);

    // History and log viewers from the old dashboard.
    let diagnostics = gtk::Expander::new(Some("History and log"));
    let diag_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let history_label = gtk::Label::new(Some(&tail_file(&Config::history_path(), 25)));
    history_label.style_context().add_class("session-meta");
    history_label.set_xalign(0.0);
    history_label.set_line_wrap(true);
    let digest = gtk::Label::new(Some(&today_digest()));
    digest.style_context().add_class("summary");
    diag_box.add(&digest);
    let audit_btn = icon_button("Export audit (history + log)", "document-save-as-symbolic");
    audit_btn.connect_clicked(|_| {
        let history = tail_file(&Config::history_path(), 1000);
        let log = tail_file(&Config::log_path(), 1000);
        let text = format!("=== NUDGE HISTORY ===\n{}\n=== KEEPER LOG ===\n{}\n", history, log);
        match std::fs::write(Config::audit_path(), &text) {
            Ok(()) => {}
            Err(e) => log_line(&format!("audit export failed: {}", e)),
        }
    });
    diag_box.add(&audit_btn);
    diag_box.add(&gtk::Label::new(Some("Recent nudges:")));
    diag_box.add(&history_label);
    let log_label = gtk::Label::new(Some(&tail_file(&Config::log_path(), 25)));
    log_label.style_context().add_class("session-meta");
    log_label.set_xalign(0.0);
    log_label.set_line_wrap(true);
    diag_box.add(&gtk::Label::new(Some("Keeper log:")));
    diag_box.add(&log_label);
    diagnostics.add(&diag_box);
    outer.add(&diagnostics);

    // Apply worker results on the UI thread.
    let rx_cell = Rc::new(RefCell::new(Some(rx)));
    let st_ui = state.clone();
    let views_ui = views.clone();
    let filter_ui = filter.clone();
    let attention_ui = attention.clone();
    let summary_ui = summary.clone();
    let busy_ui = busy.clone();
    let list_ui = list.clone();
    let history_ui = history_label.clone();
    let log_ui = log_label.clone();
    let digest_ui = digest.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = rx_cell.borrow_mut();
        if let Some(r) = guard.as_mut() {
            match r.try_recv() {
                Ok(outcome) => {
                    *st_ui.borrow_mut() = outcome.state;
                    *views_ui.borrow_mut() = outcome.views;
                    let watched = st_ui.borrow().sessions.len();
                    let attention = views_ui.borrow().values().filter(|v| needs_attention(&v.status)).count();
                    let prefix = if Config::dry_run() { "DRY RUN | " } else { "" };
                    summary_ui.set_label(&format!("{}{}  |  {} watched, {} need attention",
                        prefix, outcome.summary, watched, attention));
                    history_ui.set_label(&tail_file(&Config::history_path(), 25));
                    log_ui.set_label(&tail_file(&Config::log_path(), 25));
                    digest_ui.set_label(&today_digest());
                    refresh(&st_ui, &list_ui, &views_ui, &filter_ui, &attention_ui);
                    busy_ui.set(false);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {}
            }
        }
        glib::ControlFlow::Continue
    });

    let models_state = model_cache.clone();
    let tx_switch = switch_tx.clone();
    let summary_model = summary.clone();
    let model_rx_cell = Rc::new(RefCell::new(Some(model_rx)));
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = model_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, models)) = r.try_recv() {
                models_state.borrow_mut().insert(sid.clone(), models.clone());
                let dialog = gtk::Dialog::with_buttons(
                    Some("Switch model"),
                    None::<&gtk::Window>,
                    gtk::DialogFlags::MODAL,
                    &[("Cancel", gtk::ResponseType::Cancel), ("Switch", gtk::ResponseType::Ok)],
                );
                let combo = gtk::ComboBoxText::new();
                for m in &models {
                    let key = format!("{}|{}", m.id, m.provider);
                    let label = if m.name.is_empty() { m.id.clone() } else { m.name.clone() };
                    combo.append(Some(&key), &format!("{} ({})", label, m.provider));
                }
                if !models.is_empty() { combo.set_active(Some(0)); }
                dialog.content_area().add(&combo);
                dialog.show_all();
                let sid2 = sid.clone();
                let tx2 = tx_switch.clone();
                let _summary = summary_model.clone();
                dialog.connect_response(move |dlg, resp| {
                    if resp == gtk::ResponseType::Ok {
                        if let Some(active) = combo.active_id() {
                            let parts: Vec<&str> = active.split('|').collect();
                            if parts.len() == 2 {
                                let api = Config::api_base();
                                let pw = Config::password();
                                let sid3 = sid2.clone();
                                let tx3 = tx2.clone();
                                let id = parts[0].to_string();
                                let provider = parts[1].to_string();
                                std::thread::spawn(move || {
                                    let result = switch_model(&api, &pw, &sid3, &id, &provider);
                                    let _ = tx3.send((sid3, result));
                                });
                            }
                        }
                    }
                    dlg.close();
                });
            }
        }
        glib::ControlFlow::Continue
    });

    let switch_rx_cell = Rc::new(RefCell::new(Some(switch_rx)));
    let summary_switch = summary.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = switch_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, result)) = r.try_recv() {
                match result {
                    Ok(()) => summary_switch.set_label(&format!("Model switched for {}", short_id(&sid))),
                    Err(e) => summary_switch.set_label(&format!("Model switch failed: {}", e)),
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let kick_rx_cell = Rc::new(RefCell::new(Some(kick_rx)));
    let summary_kick = summary.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = kick_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, result)) = r.try_recv() {
                match result {
                    Ok(()) if Config::dry_run() => summary_kick.set_label(&format!("Dry-run kick for {}", short_id(&sid))),
                    Ok(()) => summary_kick.set_label(&format!("Kicked {}", short_id(&sid))),
                    Err(e) => summary_kick.set_label(&format!("Kick failed: {}", e)),
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let st_beat = state.clone();
    let beat_summary = summary.clone();
    let beat_notified: Rc<RefCell<i64>> = Rc::new(RefCell::new(0));
    glib::timeout_add_seconds_local(10, move || {
        let last = st_beat.borrow().last_tick_ms;
        if last > 0 && now_ms() - last > 120_000 {
            beat_summary.set_label(&format!("WATCHDOG STALE - last tick {}", format_age(last, now_ms())));
            let mut notified = beat_notified.borrow_mut();
            if now_ms() - *notified > 300_000 {
                notify("Session Keeper", "Watchdog has not ticked for 2 minutes");
                *notified = now_ms();
            }
        }
        glib::ControlFlow::Continue
    });

    arm_timer(&state, &tx, &busy, &timer);
    refresh(&state, &list, &views, &filter, &attention);
    window.show_all();
    launch_tick(&state, &tx, &busy);
    gtk::main();
}
