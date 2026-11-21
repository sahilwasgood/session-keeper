use glib::clone;
use gtk::prelude::*;
use rusqlite::{Connection, OpenFlags};
use session_keeper_rust::config::{Config, DRIVE};
use session_keeper_rust::db::{classify, last_idle_ts, Status};
use session_keeper_rust::nudge::{append_history, decide, log_line, send_nudge, Decision};
use session_keeper_rust::state::{Session, State};
use session_keeper_rust::ui::status_text;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    Connection::open_with_flags(Config::db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

fn tick(state: &Rc<RefCell<State>>) -> String {
    let now = now_ms();
    let conn = match open_db() {
        Some(c) => c,
        None => {
            let msg = "tick skipped: drive or db missing".to_string();
            log_line(&msg);
            return msg;
        }
    };
    let sessions: Vec<Session> = state.borrow().sessions.clone();
    let mut nudged = 0;
    let mut held = 0;
    for s in sessions {
        let (status, decision) = {
            let st = state.borrow();
            let status = classify(&conn, &s.id, &st);
            let decision = decide(&st, &s, &status, now);
            (status, decision)
        };
        match decision {
            Decision::Nudge => {
                let prompt = s
                    .prompt
                    .clone()
                    .unwrap_or_else(|| state.borrow().default_prompt.clone());
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
                        append_history(&s.id, "nudge", "ok");
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
    format!("last check {}: nudged {}, held {}", stamp(now), nudged, held)
}

fn refresh(state: &Rc<RefCell<State>>, list: &gtk::Box) {
    for child in list.children() {
        list.remove(&child);
    }
    let conn = open_db();
    let sessions = state.borrow().sessions.clone();
    for s in sessions {
        let status = conn
            .as_ref()
            .map(|c| classify(c, &s.id, &state.borrow()))
            .unwrap_or(Status::Clean);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let short: String = s.id.chars().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect();
        let label = gtk::Label::new(Some(&format!(
            "{}  {}  {}",
            short,
            status_text(&status),
            s.last_action
        )));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        let button = gtk::Button::with_label(if s.enabled { "Pause" } else { "Resume" });
        let st = state.clone();
        let list_c = list.clone();
        let sid = s.id.clone();
        button.connect_clicked(clone!(@strong st, @strong list_c, @strong sid => move |_| {
            if let Some(x) = st.borrow_mut().sessions.iter_mut().find(|x| x.id == sid) {
                x.enabled = !x.enabled;
            }
            let _ = st.borrow().save();
            refresh(&st, &list_c);
        }));
        row.add(&label);
        row.add(&button);
        list.add(&row);
    }
    list.show_all();
}

fn main() {
    gtk::init().expect("GTK3 failed to initialize");
    let state = Rc::new(RefCell::new(State::load()));

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Session Keeper");
    window.set_default_size(560, 420);
    window.connect_destroy(|_| gtk::main_quit());

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 6);
    window.add(&outer);

    let status = gtk::Label::new(Some("starting"));
    status.set_xalign(0.0);
    outer.add(&status);

    let note = gtk::Label::new(Some("On demand only: the watchdog runs while this window is open."));
    note.set_xalign(0.0);
    outer.add(&note);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.add(&list);
    scroll.set_vexpand(true);
    outer.add(&scroll);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("session id (ses_...)"));
    entry.set_hexpand(true);
    controls.add(&entry);

    let add = gtk::Button::with_label("Add");
    let st = state.clone();
    let list_add = list.clone();
    let status_add = status.clone();
    let entry_add = entry.clone();
    add.connect_clicked(clone!(@strong st, @strong list_add, @strong status_add, @strong entry_add => move |_| {
        let sid = entry_add.text().trim().to_string();
        if sid.starts_with("ses_") && !st.borrow().sessions.iter().any(|s| s.id == sid) {
            st.borrow_mut().sessions.push(Session {
                id: sid,
                enabled: true,
                prompt: None,
                handled: 0,
                prompted: 0,
                last_nudge: 0,
                nudges: 0,
                last_action: "added".to_string(),
            });
            let _ = st.borrow().save();
            entry_add.set_text("");
            status_add.set_label("session added");
            refresh(&st, &list_add);
        }
    }));
    controls.add(&add);

    let check = gtk::Button::with_label("Check now");
    let st = state.clone();
    let list_check = list.clone();
    let status_check = status.clone();
    check.connect_clicked(clone!(@strong st, @strong list_check, @strong status_check => move |_| {
        status_check.set_label(&tick(&st));
        refresh(&st, &list_check);
    }));
    controls.add(&check);

    let quit = gtk::Button::with_label("Quit");
    quit.connect_clicked(|_| gtk::main_quit());
    controls.add(&quit);
    outer.add(&controls);

    refresh(&state, &list);
    status.set_label(&tick(&state));
    refresh(&state, &list);

    let st = state.clone();
    let list_timer = list.clone();
    let status_timer = status.clone();
    gtk::glib::timeout_add_seconds_local(30, move || {
        status_timer.set_label(&tick(&st));
        refresh(&st, &list_timer);
        glib::ControlFlow::Continue
    });

    window.show_all();
    gtk::main();
}
