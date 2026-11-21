use glib::clone;
use gtk::prelude::*;
use rusqlite::{Connection, OpenFlags};
use session_keeper_rust::config::{Config, DRIVE};
use session_keeper_rust::db::{classify, last_idle_ts, session_title, Status};
use session_keeper_rust::nudge::{append_history, decide, log_line, send_nudge, Decision};
use session_keeper_rust::state::{Session, State};
use session_keeper_rust::ui::{short_id, status_class, status_text, summary_line, CSS};
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
    Connection::open_with_flags(Config::db_path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .ok()
        .and_then(|c| {
            // Keep a locked DB from stalling the GTK main loop for the default 5s.
            c.busy_timeout(std::time::Duration::from_millis(1200)).ok()?;
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
            let decision = decide(&st, &s, &status, now);
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
    summary_line(&stamp(now), nudged, held)
}

fn refresh(state: &Rc<RefCell<State>>, list: &gtk::Box) {
    for child in list.children() {
        list.remove(&child);
    }
    let conn = open_db();
    let sessions = state.borrow().sessions.clone();
    for s in &sessions {
        let (status, title) = conn
            .as_ref()
            .map(|c| (classify(c, &s.id, &state.borrow()), session_title(c, &s.id)))
            .unwrap_or((Status::Clean, String::new()));

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.style_context().add_class("card");

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let badge = gtk::Label::new(Some(status_text(&status)));
        badge.style_context().add_class("badge");
        badge.style_context().add_class(status_class(&status));
        badge.set_size_request(120, -1);
        top.add(&badge);

        let name = if title.is_empty() { short_id(&s.id) } else { format!("{}   {}", title, short_id(&s.id)) };
        let id_label = gtk::Label::new(Some(&name));
        id_label.style_context().add_class("session-id");
        id_label.set_xalign(0.0);
        id_label.set_hexpand(true);
        id_label.set_tooltip_text(Some(&s.id));
        top.add(&id_label);

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
        top.add(&button);
        card.add(&top);

        let meta = gtk::Label::new(Some(&format!(
            "nudges {}   last: {}   prompt: {}",
            s.nudges,
            if s.last_action.is_empty() { "never" } else { &s.last_action },
            s.prompt.as_deref().unwrap_or("default")
        )));
        meta.style_context().add_class("session-meta");
        meta.set_xalign(0.0);
        meta.set_line_wrap(true);
        card.add(&meta);

        list.add(&card);
    }
    if sessions.is_empty() {
        let empty = gtk::Label::new(Some("No watched sessions. Add a session id below."));
        empty.style_context().add_class("note");
        list.add(&empty);
    }
    list.show_all();
}

fn main() {
    gtk::init().expect("GTK3 failed to initialize");

    if let Some(screen) = gtk::gdk::Screen::default() {
        let provider = gtk::CssProvider::new();
            {
            let _ = provider.load_from_data(CSS.as_bytes());
            gtk::StyleContext::add_provider_for_screen(
                &screen,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    let state = Rc::new(RefCell::new(State::load()));

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Session Keeper");
    window.set_default_size(920, 620);
    window.set_size_request(700, 480);
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

    let summary = gtk::Label::new(Some("starting"));
    summary.style_context().add_class("summary");
    summary.set_xalign(0.0);
    outer.add(&summary);

    let note = gtk::Label::new(Some("The watchdog runs only while this window is open. Closing it stops everything."));
    note.style_context().add_class("note");
    note.set_xalign(0.0);
    outer.add(&note);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("Session id (ses_...)"));
    entry.set_hexpand(true);
    controls.add(&entry);

    let add = gtk::Button::with_label("Add session");
    controls.add(&add);

    let check = gtk::Button::with_label("Check now");
    check.style_context().add_class("suggested-action");
    controls.add(&check);
    outer.add(&controls);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_vexpand(true);
    scroll.add(&list);
    outer.add(&scroll);

    // Wire the controls now that `list` exists.
    let st_add = state.clone();
    let list_add = list.clone();
    let summary_add = summary.clone();
    let entry_add = entry.clone();
    add.connect_clicked(clone!(@strong st_add, @strong list_add, @strong summary_add, @strong entry_add => move |_| {
        let sid = entry_add.text().trim().to_string();
        if sid.starts_with("ses_") && !st_add.borrow().sessions.iter().any(|s| s.id == sid) {
            st_add.borrow_mut().sessions.push(Session {
                id: sid,
                enabled: true,
                prompt: None,
                handled: 0,
                prompted: 0,
                last_nudge: 0,
                nudges: 0,
                last_action: "added".to_string(),
            });
            let _ = st_add.borrow().save();
            entry_add.set_text("");
            summary_add.set_label("Session added");
            refresh(&st_add, &list_add);
        }
    }));

    let st_check = state.clone();
    let list_check = list.clone();
    let summary_check = summary.clone();
    check.connect_clicked(clone!(@strong st_check, @strong list_check, @strong summary_check => move |_| {
        summary_check.set_label(&tick(&st_check));
        refresh(&st_check, &list_check);
    }));

    refresh(&state, &list);
    summary.set_label(&tick(&state));
    refresh(&state, &list);

    let st_timer = state.clone();
    let list_timer = list.clone();
    let summary_timer = summary.clone();
    glib::timeout_add_seconds_local(30, move || {
        summary_timer.set_label(&tick(&st_timer));
        refresh(&st_timer, &list_timer);
        glib::ControlFlow::Continue
    });

    window.show_all();
    gtk::main();
}

