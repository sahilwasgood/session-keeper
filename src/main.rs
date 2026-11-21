use glib::clone;
use gtk::prelude::*;
use rusqlite::{Connection, OpenFlags};
use session_keeper_rust::config::{Config, DRIVE};
use session_keeper_rust::db::{classify, last_idle_ts, session_title, Status};
use session_keeper_rust::nudge::{append_history, decide, log_line, send_nudge, Decision};
use session_keeper_rust::state::{Session, State};
use session_keeper_rust::ui::{prompt_display, short_id, status_class, status_text, summary_line, CSS};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct SessionView {
    status: Status,
    title: String,
}

struct TickOutcome {
    summary: String,
    state: State,
    views: HashMap<String, SessionView>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn stamp(ms: i64) -> String {
    let secs = (ms / 1000) % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

fn open_db() -> Option<Connection> {
    if !Path::new(DRIVE).exists() || !Config::db_path().exists() {
        return None;
    }
    Connection::open_with_flags(Config::db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .and_then(|c| {
            c.busy_timeout(Duration::from_millis(1200)).ok()?;
            Some(c)
        })
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
    let mut nudged = 0;
    let mut held = 0;
    for s in &sessions {
        let (status, decision) = {
            let st = state.borrow();
            let status = classify(&conn, &s.id, &st);
            let decision = decide(&st, s, &status, now);
            (status, decision)
        };
        match decision {
            Decision::Nudge => {
                let raw = s.prompt.clone().unwrap_or_else(|| state.borrow().default_prompt.clone());
                let prompt = raw.trim().to_string();
                let prompt = if prompt.is_empty() { "continue".to_string() } else { prompt };
                match send_nudge(Config::api_base(), &Config::password(), &s.id, &prompt) {
                    Ok(()) => {
                        let ts = last_idle_ts(&conn, &s.id);
                        {
                            let mut st = state.borrow_mut();
                            if let Some(x) = st.sessions.iter_mut().find(|x| x.id == s.id) {
                                x.handled = ts;
                                x.prompted = now;
                                x.last_nudge = now;
                                x.nudges += 1;
                                x.last_action = format!("nudged {}", stamp(now));
                            }
                        }
                        append_history(&s.id, "nudge", &prompt);
                        log_line(&format!("{}: prompted", s.id));
                        nudged += 1;
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
            Decision::Hold => held += 1,
            Decision::Skip => {}
        }
        let _ = status;
    }
    if nudged > 0 {
        let _ = state.borrow().save();
    }
    summary_line(&stamp(now), nudged, held)
}

fn run_tick(snapshot: State) -> TickOutcome {
    // Runs on a worker thread. No GTK calls here.
    let st = Rc::new(RefCell::new(snapshot));
    let summary = tick(&st);
    let mut views = HashMap::new();
    if let Some(conn) = open_db() {
        for s in st.borrow().sessions.iter() {
            let view = SessionView {
                status: classify(&conn, &s.id, &st.borrow()),
                title: session_title(&conn, &s.id),
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
) {
    for child in list.children() {
        list.remove(&child);
    }
    let sessions = state.borrow().sessions.clone();
    let default_prompt = state.borrow().default_prompt.clone();
    let view_map = views.borrow();
    for s in &sessions {
        let view = view_map.get(&s.id);
        let status = view.map(|v| v.status).unwrap_or(Status::Clean);
        let title = view.map(|v| v.title.as_str()).unwrap_or("");

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.style_context().add_class("card");

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let badge = gtk::Label::new(Some(status_text(&status)));
        badge.style_context().add_class("badge");
        badge.style_context().add_class(status_class(&status));
        badge.set_size_request(118, -1);
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
        let sid = s.id.clone();
        pause.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid => move |_| {
            if let Some(x) = st.borrow_mut().sessions.iter_mut().find(|x| x.id == sid) {
                x.enabled = !x.enabled;
            }
            let _ = st.borrow().save();
            refresh(&st, &list_c, &views_c);
        }));
        top.add(&pause);
        card.add(&top);

        let meta = gtk::Label::new(Some(&format!(
            "prompt: {}   nudges {}   last: {}",
            prompt_display(s.prompt.as_deref(), &default_prompt),
            s.nudges,
            if s.last_action.is_empty() { "never" } else { &s.last_action }
        )));
        meta.style_context().add_class("session-meta");
        meta.set_xalign(0.0);
        meta.set_line_wrap(true);
        card.add(&meta);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let edit = gtk::Button::with_label("Edit prompt");
        let st = state.clone();
        let list_c = list.clone();
        let views_c = views.clone();
        let sid_edit = s.id.clone();
        let cur = s.prompt.clone();
        edit.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid_edit, @strong cur => move |_| {
            edit_prompt(&st, &list_c, &views_c, &sid_edit, cur.as_deref());
        }));
        actions.add(&edit);

        let del = gtk::Button::with_label("Remove");
        let st = state.clone();
        let list_c = list.clone();
        let views_c = views.clone();
        let sid_del = s.id.clone();
        del.connect_clicked(clone!(@strong st, @strong list_c, @strong views_c, @strong sid_del => move |_| {
            {
                let mut st_mut = st.borrow_mut();
                st_mut.sessions.retain(|x| x.id != sid_del);
                let _ = st_mut.save();
            }
            refresh(&st, &list_c, &views_c);
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
            refresh(&st, &list_c, &views_c);
        }
        dlg.close();
    });
}

fn main() {
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
    let views: Rc<RefCell<HashMap<String, SessionView>>> = Rc::new(RefCell::new(HashMap::new()));
    let busy = Rc::new(Cell::new(false));
    let (tx, rx) = channel::<TickOutcome>();

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Session Keeper");
    window.set_default_size(960, 680);
    window.set_size_request(760, 520);
    window.connect_destroy(|_| gtk::main_quit());

    let header = gtk::HeaderBar::new();
    header.set_title(Some("Session Keeper"));
    header.set_subtitle(Some("On-demand OpenCode watchdog"));
    header.set_show_close_button(true);
    window.set_titlebar(Some(&header));
    let quit = gtk::Button::with_label("Quit");
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

    let add = gtk::Button::with_label("Add session");
    let st_add = state.clone();
    let list_add = Rc::new(RefCell::new(gtk::Box::new(gtk::Orientation::Vertical, 2)));
    let views_add = views.clone();
    let summary_add = summary.clone();
    let entry_add = entry.clone();
    add.connect_clicked(clone!(@strong st_add, @strong list_add, @strong views_add, @strong summary_add, @strong entry_add => move |_| {
        let sid = entry_add.text().trim().to_string();
        if sid.starts_with("ses_") && !st_add.borrow().sessions.iter().any(|s| s.id == sid) {
            st_add.borrow_mut().sessions.push(Session {
                id: sid, enabled: true, prompt: None, handled: 0, prompted: 0,
                last_nudge: 0, nudges: 0, last_action: "added".to_string(),
            });
            let _ = st_add.borrow().save();
            entry_add.set_text("");
            summary_add.set_label("Session added");
            refresh(&st_add, &list_add.borrow(), &views_add);
        }
    }));
    controls.add(&add);

    let check = gtk::Button::with_label("Check now");
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

    let save_settings = gtk::Button::with_label("Save settings");
    save_settings.style_context().add_class("suggested-action");
    let st_settings = state.clone();
    save_settings.connect_clicked(move |_| {
        let mut st = st_settings.borrow_mut();
        let p = def_prompt.text().trim().to_string();
        st.default_prompt = if p.is_empty() { "continue, dont stop".to_string() } else { p };
        st.min_gap = min_gap.value() as i64;
        st.max_nudges_per_hour = cap.value() as i64;
        st.away_mode = away.is_active();
        let _ = st.save();
    });
    settings_box.add(&save_settings);
    settings.add(&settings_box);
    outer.add(&settings);

    // History and log viewers from the old dashboard.
    let diagnostics = gtk::Expander::new(Some("History and log"));
    let diag_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let history_label = gtk::Label::new(Some(&tail_file(&Config::history_path(), 25)));
    history_label.style_context().add_class("session-meta");
    history_label.set_xalign(0.0);
    history_label.set_line_wrap(true);
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
    let summary_ui = summary.clone();
    let busy_ui = busy.clone();
    let list_ui = list.clone();
    let history_ui = history_label.clone();
    let log_ui = log_label.clone();
    glib::timeout_add_local(Duration::from_millis(150), move || {
        let mut guard = rx_cell.borrow_mut();
        if let Some(r) = guard.as_mut() {
            match r.try_recv() {
                Ok(outcome) => {
                    *st_ui.borrow_mut() = outcome.state;
                    *views_ui.borrow_mut() = outcome.views;
                    summary_ui.set_label(&outcome.summary);
                    history_ui.set_label(&tail_file(&Config::history_path(), 25));
                    log_ui.set_label(&tail_file(&Config::log_path(), 25));
                    refresh(&st_ui, &list_ui, &views_ui);
                    busy_ui.set(false);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {}
            }
        }
        glib::ControlFlow::Continue
    });

    let st_timer = state.clone();
    let tx_timer = tx.clone();
    let busy_timer = busy.clone();
    glib::timeout_add_seconds_local(30, move || {
        launch_tick(&st_timer, &tx_timer, &busy_timer);
        glib::ControlFlow::Continue
    });

    refresh(&state, &list, &views);
    window.show_all();
    launch_tick(&state, &tx, &busy);
    gtk::main();
}
