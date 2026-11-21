use glib::clone;
use gtk::prelude::*;
use rusqlite::{Connection, OpenFlags};
use session_keeper_rust::cli::{parse_args, Mode, HELP};
use session_keeper_rust::config::{load_env_file, Config};
use session_keeper_rust::events::watch_events;
use session_keeper_rust::db::{classify, last_assistant_text, last_assistant_ts, last_idle_ts, session_exists, session_model, session_title, Status};
use session_keeper_rust::nudge::{append_history, decide, fetch_models_result, fetch_sessions_result, has_pending_human_input, log_line, notify, notify_send_available, nudges_in_last_hour_all, send_nudge, switch_model, Decision, ModelInfo, SessionInfo};
use session_keeper_rust::process::create_lock;
use session_keeper_rust::state::{apply_tick_fields, states_differ, GlobalTickFields, Session, State, TickFields};
use session_keeper_rust::safety::{fingerprint, is_repeating_output, should_nudge_after_debounce};
use session_keeper_rust::ui::{format_age, history_for_sid, matches_filter, needs_attention, prompt_display, short_id, sort_key, status_changed, status_class, status_text, summary_line, CSS};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static MODEL_TX: OnceLock<Sender<(String, Result<Vec<ModelInfo>, String>)>> = OnceLock::new();
static KICK_TX: OnceLock<Sender<(String, i64, Result<(), String>)>> = OnceLock::new();
static ADD_TX: OnceLock<Sender<(String, bool)>> = OnceLock::new();
static SESSION_TX: OnceLock<Sender<Result<Vec<SessionInfo>, String>>> = OnceLock::new();

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
    updates: Vec<TickFields>,
    globals: GlobalTickFields,
    views: HashMap<String, SessionView>,
    error: Option<String>,
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

fn tick(state: &Rc<RefCell<State>>, persist: bool) -> (String, Vec<TickFields>, GlobalTickFields) {
    let now = now_ms();
    let conn = match open_db() {
        Some(c) => c,
        None => {
            let msg = "Watchdog paused: drive or database unavailable".to_string();
            log_line(&msg);
            return (msg, Vec::new(), GlobalTickFields {
                // Keep the old heartbeat: a paused watchdog must still trip the
                // stale detector if the database stays unavailable.
                last_tick_ms: state.borrow().last_tick_ms,
                cap_alert_ms: state.borrow().cap_alert_ms,
                missing_alert_ms: state.borrow().missing_alert_ms,
                cap_remaining: -1,
            });
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

        if !session_exists(&conn, &s.id) {
            let last = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.missing_alert_ms).unwrap_or(0);
            if now - last > 3_600_000 {
                notify("Session Keeper", &format!("Watched session {} is missing from OpenCode", short_id(&s.id)));
                if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
                    x.missing_alert_ms = now;
                }
                if persist { let _ = state.borrow().save(); }
                log_line(&format!("{}: alerted missing from opencode", s.id));
            }
        }

        let status_name = format!("{:?}", status);
        if status == Status::Waiting {
            let prev = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.last_status.clone()).unwrap_or_default();
            let last_alert = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.question_alert_ms).unwrap_or(0);
            let cooldown = state.borrow().alert_cooldown_s * 1000;
            if status_changed(&prev, &status_name) && now - last_alert > cooldown {
                let title = session_title(&conn, &s.id);
                notify("Session Keeper", &format!("{} asked a question", if title.is_empty() { &s.id } else { &title }));
                if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
                    x.question_alert_ms = now;
                }
                log_line(&format!("{}: alerted waiting on question", s.id));
            }
        }
        if status == Status::Stalled {
            let prev = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.last_status.clone()).unwrap_or_default();
            let last_alert = state.borrow().sessions.iter().find(|x| x.id == s.id)
                .map(|x| x.stalled_alert_ms).unwrap_or(0);
            let cooldown = state.borrow().alert_cooldown_s * 1000;
            if status_changed(&prev, &status_name) && now - last_alert > cooldown {
                let title = session_title(&conn, &s.id);
                notify("Session Keeper", &format!("{} is stalled after a nudge", if title.is_empty() { &s.id } else { &title }));
                if let Some(x) = state.borrow_mut().sessions.iter_mut().find(|x| x.id == s.id) {
                    x.stalled_alert_ms = now;
                }
                log_line(&format!("{}: alerted stalled after nudge", s.id));
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
        let (last_hash, prev_hash) = state
            .borrow()
            .sessions
            .iter()
            .find(|x| x.id == s.id)
            .map(|x| (x.last_hash.clone(), x.prev_hash.clone()))
            .unwrap_or((None, None));
        if is_repeating_output(&fp, last_hash.as_deref(), prev_hash.as_deref()) {
            log_line(&format!("{}: hold loop (repeating output)", s.id));
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
            let last_alert = state.borrow().cap_alert_ms;
            if now - last_alert > 3_600_000 {
                notify("Session Keeper", &format!("Global nudge cap reached ({}/hour)", global_cap));
                state.borrow_mut().cap_alert_ms = now;
                let _ = state.borrow().save();
            }
            held += 1;
            continue;
        }

        if Config::dry_run() {
            log_line(&format!("{}: would nudge (dry run)", s.id));
            held += 1;
            continue;
        }

        if has_pending_human_input(&Config::api_base(), &Config::password(), &s.id) {
            log_line(&format!("{}: hold pending_human_input", s.id));
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
                        x.prev_hash = x.last_hash.take();
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
                let cooldown = st.alert_cooldown_s * 1000;
                if let Some(x) = st.sessions.iter_mut().find(|x| x.id == s.id) {
                    x.last_action = format!("failed {}", e);
                    if now - x.failure_alert_ms > cooldown {
                        let title = session_title(&conn, &s.id);
                        notify(
                            "Session Keeper",
                            &format!("Nudge failed for {}", if title.is_empty() { &s.id } else { &title }),
                        );
                        x.failure_alert_ms = now;
                        log_line(&format!("{}: alerted nudge failure", s.id));
                    }
                }
                log_line(&format!("{}: prompt failed {}", s.id, e));
            }
        }
    }

    state.borrow_mut().last_tick_ms = now;
    if persist { let _ = state.borrow().save(); }
    let updates = state.borrow().sessions.iter().map(|s| TickFields {
        id: s.id.clone(),
        handled: s.handled,
        prompted: s.prompted,
        last_nudge: s.last_nudge,
        nudges: s.nudges,
        idle_streak: s.idle_streak,
        last_hash: s.last_hash.clone(),
        prev_hash: s.prev_hash.clone(),
        last_action: s.last_action.clone(),
        last_status: s.last_status.clone(),
        last_alert_ms: s.last_alert_ms,
        question_alert_ms: s.question_alert_ms,
        stalled_alert_ms: s.stalled_alert_ms,
        missing_alert_ms: s.missing_alert_ms,
        failure_alert_ms: s.failure_alert_ms,
    }).collect();
    let globals = GlobalTickFields {
        last_tick_ms: now,
        cap_alert_ms: state.borrow().cap_alert_ms,
        missing_alert_ms: state.borrow().missing_alert_ms,
        cap_remaining: if global_cap > 0 { (global_cap - used_global).max(0) } else { -1 },
    };
    (summary_line(&session_keeper_rust::config::format_log_time(now / 1000), nudged, held), updates, globals)
}

fn run_tick(snapshot: State, persist: bool) -> TickOutcome {
    // Runs on a worker thread. No GTK calls here.
    let st = Rc::new(RefCell::new(snapshot));
    let (summary, updates, globals) = tick(&st, persist);
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
    TickOutcome { summary, updates, globals, views, error: None }
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_tick(snapshot, false)));
        let outcome = match result {
            Ok(o) => o,
            Err(_) => {
                log_line("Watchdog error: tick failed, see log for details");
                TickOutcome {
                    summary: "Watchdog error: tick failed".to_string(),
                    updates: Vec::new(),
                    globals: GlobalTickFields { last_tick_ms: now_ms(), cap_alert_ms: 0, missing_alert_ms: 0, cap_remaining: -1 },
                    views: HashMap::new(),
                    error: Some("tick failed".to_string()),
                }
            }
        };
        let _ = tx.send(outcome);
    });
}

fn today_digest() -> String {
    let now = now_ms();
    let day_start = session_keeper_rust::config::local_day_start_ms(now);
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
    let by_name = state.borrow().sort_by_name;
    let view_map = views.borrow();
    let filter_text = filter.borrow();
    let attention_on = *attention.borrow();
    let mut items: Vec<_> = sessions.iter().map(|s| (s, view_map.get(&s.id))).collect();
    if attention_on {
        items.retain(|(_, v)| v.map(|x| needs_attention(&x.status)).unwrap_or(false));
    }
    items.sort_by_key(|(s, v)| {
        sort_key(
            &v.map(|x| x.status).unwrap_or(Status::Clean),
            v.map(|x| x.title.as_str()).unwrap_or(""),
            &s.id,
            by_name,
        )
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
        if let Some(v) = view {
            badge.set_tooltip_text(Some(&session_keeper_rust::ui::status_tooltip(&v.status)));
        }
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

        let session_cap = st.borrow().max_nudges_per_hour;
        let nudge_text = if session_cap > 0 {
            format!("{}/{}", s.nudges, session_cap)
        } else {
            format!("{}", s.nudges)
        };
        let meta = gtk::Label::new(Some(&format!(
            "prompt: {}   nudges {}   last: {}",
            {
                let p = prompt_display(s.prompt.as_deref(), &default_prompt);
                if p.chars().count() > 60 {
                    format!("{}...", p.chars().take(60).collect::<String>())
                } else {
                    p
                }
            },
            nudge_text,
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
        let details = icon_button("Details", "help-about-symbolic");
        let sid_details = s.id.clone();
        let st_details = state.clone();
        let views_details = views.clone();
        details.connect_clicked(move |_| {
            let sid = sid_details.clone();
            let s = st_details.borrow().sessions.iter().find(|x| x.id == sid).cloned();
            let v = views_details.borrow().get(&sid).cloned();
            let body = match (s, v) {
                (Some(sess), Some(view)) => format!(
                    "Session: {}\nStatus: {} ({})\nModel: {}\nPrompt: {}\nNudges: {}\nLast action: {}\nLast progress: {}\nIdle since: {}\nPresent in OpenCode: {}",
                    sess.id,
                    status_text(&view.status),
                    view.reason,
                    if view.model.is_empty() { "unknown" } else { &view.model },
                    prompt_display(sess.prompt.as_deref(), &st_details.borrow().default_prompt),
                    sess.nudges,
                    if sess.last_action.is_empty() { "never" } else { &sess.last_action },
                    format_age(view.progress_ms, now_ms()),
                    if view.idle_ms > 0 { format_age(view.idle_ms, now_ms()) } else { "none".to_string() },
                    view.exists,
                ),
                _ => "Session data unavailable".to_string(),
            };
            let history_text = std::fs::read_to_string(Config::history_path()).unwrap_or_default();
            let recent = history_for_sid(&history_text, &sid, 8).join("\n");
            let body = format!("{}\n\nRecent nudges:\n{}", body, if recent.is_empty() { "none" } else { &recent });
            let dlg = gtk::Dialog::with_buttons(
                Some("Session details"),
                None::<&gtk::Window>,
                gtk::DialogFlags::MODAL,
                &[("Close", gtk::ResponseType::Close)],
            );
            let label = gtk::Label::new(Some(&body));
            label.set_xalign(0.0);
            label.set_line_wrap(true);
            label.set_selectable(true);
            dlg.content_area().add(&label);
            dlg.show_all();
            dlg.connect_response(|dlg, _| dlg.close());
        });
        actions.add(&details);

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
                let models = fetch_models_result(&api, &pw);
                let _ = tx.send((sid, models));
            });
        });
        actions.add(&model_btn);

        let kick = icon_button("Kick", "mail-send-symbolic");
        let kick_cooldown = kick.clone();
        let sid_kick = s.id.clone();
        let prompt_kick = s.prompt.clone().unwrap_or_default();
        let default_kick = default_prompt.clone();
        kick.connect_clicked(move |_| {
            let dialog = gtk::Dialog::with_buttons(
                Some("Kick now"),
                None::<&gtk::Window>,
                gtk::DialogFlags::MODAL,
                &[("Cancel", gtk::ResponseType::Cancel), ("Kick", gtk::ResponseType::Ok)],
            );
            let label = gtk::Label::new(Some(
                "Kick bypasses debounce, caps and question guards. Send the nudge now?",
            ));
            label.set_line_wrap(true);
            dialog.content_area().add(&label);
            dialog.show_all();

            let cool = kick_cooldown.clone();
            let tx = match KICK_TX.get() { Some(t) => t.clone(), None => return };
            let sid = sid_kick.clone();
            let prompt = if prompt_kick.trim().is_empty() { default_kick.clone() } else { prompt_kick.clone() };
            dialog.connect_response(move |dlg, resp| {
                if resp == gtk::ResponseType::Ok {
                    cool.set_sensitive(false);
                    let cool2 = cool.clone();
                    glib::timeout_add_seconds_local(10, move || {
                        cool2.set_sensitive(true);
                        glib::ControlFlow::Break
                    });
                    let sid2 = sid.clone();
                    let prompt2 = prompt.clone();
                    let tx2 = tx.clone();
                    let api = Config::api_base();
                    let pw = Config::password();
                    std::thread::spawn(move || {
                        let now = now_ms();
                        let ts = open_db().map(|c| last_idle_ts(&c, &sid2)).unwrap_or(now);
                        let result = if Config::dry_run() {
                            Ok(())
                        } else {
                            send_nudge(&api, &pw, &sid2, &prompt2)
                        };
                        if result.is_ok() && !Config::dry_run() {
                            append_history(&sid2, "nudge", &prompt2);
                            log_line(&format!("{}: kicked from UI", sid2));
                        }
                        let _ = tx2.send((sid2, ts, result));
                    });
                }
                dlg.close();
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
        let empty = gtk::Label::new(Some("No watched sessions. Add a session id in the field above."));
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

fn print_line(line: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(line.as_bytes());
    let _ = stdout.write_all(b"\n");
}

fn write_audit() -> std::io::Result<std::path::PathBuf> {
    let history = tail_file(&Config::history_path(), 1000);
    let log = tail_file(&Config::log_path(), 1000);
    let settings = serde_json::to_string_pretty(&State::load()).unwrap_or_default();
    let text = format!(
        "=== SETTINGS ===\n{}\n=== NUDGE HISTORY ===\n{}\n=== KEEPER LOG ===\n{}\n",
        settings, history, log
    );
    std::fs::write(Config::audit_path(), &text)?;
    Ok(Config::audit_path())
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

fn preflight() {
    let db = Config::db_path();
    log_line(&format!("Preflight: database {} {}", db.display(), if db.exists() { "ok" } else { "MISSING" }));
    let svc = Config::services_json();
    log_line(&format!("Preflight: credentials {} {}", svc.display(), if svc.exists() { "ok" } else { "MISSING" }));
    log_line(&format!("Preflight: notification transport {}", if notify_send_available() { "ok" } else { "MISSING notify-send" }));
}

fn main() {
    load_env_file();
    let raw: Vec<String> = std::env::args().collect();
    let opts = parse_args(&raw);
    if matches!(opts.mode, Mode::Gui | Mode::Watch) {
        preflight();
    }
    let boot_state = State::load();
    if session_keeper_rust::state::state_was_corrupt() {
        let msg = "State file was corrupt: a timestamped backup was saved and defaults were loaded";
        eprintln!("session-keeper: {}", msg);
        log_line(msg);
        let _ = notify("Session Keeper", msg);
    }
    if std::env::var_os("SESSION_KEEPER_DRY_RUN").is_none() {
        std::env::set_var(
            "SESSION_KEEPER_DRY_RUN",
            if boot_state.persist_dry_run { "1" } else { "0" },
        );
    }
    if std::env::var_os("SESSION_KEEPER_NOTIFY").is_none() {
        std::env::set_var(
            "SESSION_KEEPER_NOTIFY",
            if boot_state.persist_notifications { "1" } else { "0" },
        );
    }
    if opts.dry_run {
        std::env::set_var("SESSION_KEEPER_DRY_RUN", "1");
    }
    let resume_mode = matches!(&opts.mode, Mode::Resume(_));
    let lock_path = Config::lock_path();
    let lock_needed = matches!(
        opts.mode,
        Mode::Gui
            | Mode::CheckNow
            | Mode::Watch
            | Mode::Kick(_)
            | Mode::SetModel(..)
            | Mode::Pause(_)
            | Mode::Resume(_)
            | Mode::SetPrompt(..)
            | Mode::ImportState(_)
    );
    if lock_needed {
        if let Err(e) = create_lock(&lock_path, std::process::id() as i32) {
            eprintln!("Session Keeper cannot start: {}", e);
            return;
        }
    }
    let _lock_guard = if lock_needed { Some(LockGuard(lock_path)) } else { None };

    match opts.mode {
        Mode::Help => {
            print!("{}", HELP);
            return;
        }
        Mode::CheckNow => {
            let st = Rc::new(RefCell::new(State::load()));
            let out = run_tick(st.borrow().clone(), true);
            print_line(&format!("{}", out.summary));
            return;
        }
        Mode::Status => {
            let st = State::load();
            let conn = open_db();
            if opts.json {
                let items: Vec<serde_json::Value> = st.sessions.iter().map(|s| {
                    let status = conn.as_ref().map(|c| classify(c, &s.id, &st)).unwrap_or(Status::Clean);
                    let model = conn.as_ref().map(|c| session_model(c, &s.id)).unwrap_or_default();
                    serde_json::json!({
                        "id": s.id,
                        "enabled": s.enabled,
                        "status": status_text(&status),
                        "reason": reason(&status),
                        "model": model,
                        "nudges": s.nudges,
                        "last_action": s.last_action,
                        "last_nudge_ms": s.last_nudge,
                        "min_gap_s": st.min_gap,
                        "session_cap_per_hour": st.max_nudges_per_hour,
                        "away_mode": st.away_mode,
                    })
                }).collect();
                print_line(&serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string()));
                return;
            }
            for s in &st.sessions {
                let status = conn.as_ref().map(|c| classify(c, &s.id, &st)).unwrap_or(Status::Clean);
                let model = conn.as_ref().map(|c| session_model(c, &s.id)).unwrap_or_default();
                let last_age = if s.last_nudge > 0 { format_age(s.last_nudge, now_ms()) } else { "never".to_string() };
                print_line(&format!("{}  {}  {} ({})  model={}  nudges={}  last nudge {} ago  last={}",
                    short_id(&s.id), s.id, status_text(&status), reason(&status),
                    if model.is_empty() { "-" } else { &model }, s.nudges, last_age, s.last_action));
            }
            return;
        }
        Mode::Version => {
            print_line(&format!("Session Keeper {} (Rust)", env!("CARGO_PKG_VERSION")));
            return;
        }
        Mode::Doctor => {
            print_line(&format!("Session Keeper {} (Rust)", env!("CARGO_PKG_VERSION")));
            let db = Config::db_path();
            print_line(&format!("database: {} [{}]", db.display(), if db.exists() { "ok" } else { "MISSING" }));
            let svc = Config::services_json();
            print_line(&format!("credentials: {} [{}]", svc.display(), if svc.exists() { "ok" } else { "MISSING" }));
            print_line(&format!("notifier: {}", if notify_send_available() { "notify-send ok" } else { "MISSING notify-send" }));
            print_line(&format!("api: {}", Config::api_base()));
            match fetch_sessions_result(&Config::api_base(), &Config::password()) {
                Ok(sessions) => print_line(&format!("api status: reachable ({} sessions)", sessions.len())),
                Err(e) => print_line(&format!("api status: UNREACHABLE ({})", e)),
            }
            return;
        }
        Mode::ExportState(path) => {
            let st = State::load();
            match std::fs::write(&path, serde_json::to_string_pretty(&st).unwrap_or_default()) {
                Ok(()) => println!("State exported to {}", path),
                Err(e) => {
                    eprintln!("State export failed: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::Sessions => {
            match fetch_sessions_result(&Config::api_base(), &Config::password()) {
                Ok(sessions) => for s in sessions {
                    print_line(&format!("{}  {}", s.id, s.title));
                },
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::SetSetting(key, value) => {
            let mut st = State::load();
            let parsed = match key.as_str() {
                "min_gap" | "cap" | "global_cap" | "interval" | "cooldown" => value.parse::<i64>().ok(),
                "away" | "enabled" | "dry_run" | "notify" => match value.as_str() {
                    "1" | "true" | "on" => Some(1),
                    "0" | "false" | "off" => Some(0),
                    _ => None,
                },
                _ => None,
            };
            let number = match parsed {
                Some(n) => n,
                None => {
                    eprintln!("Unknown setting or invalid value: {} = {}", key, value);
                    std::process::exit(1);
                }
            };
            match key.as_str() {
                "min_gap" => st.min_gap = number,
                "cap" => st.max_nudges_per_hour = number,
                "global_cap" => st.global_nudge_cap = number,
                "interval" => st.tick_interval_s = number,
                "cooldown" => st.alert_cooldown_s = number,
                "away" => st.away_mode = number != 0,
                "enabled" => st.enabled = number != 0,
                "dry_run" => st.persist_dry_run = number != 0,
                "notify" => st.persist_notifications = number != 0,
                _ => unreachable!(),
            }
            let text = serde_json::to_string(&st).unwrap_or_default();
            let clean = State::from_json(&text).unwrap_or(st);
            if let Err(e) = clean.save() {
                eprintln!("Setting save failed: {}", e);
                std::process::exit(1);
            }
            print_line(&format!("{} = {}", key, value));
            return;
        }
        Mode::ImportState(path) => {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("State import failed: {}", e);
                    std::process::exit(1);
                }
            };
            let imported: State = match State::from_json(&text) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Invalid state file: {}", e);
                    std::process::exit(1);
                }
            };
            if let Some(backup) = session_keeper_rust::state::backup_state() {
                println!("Backup saved to {}", backup.display());
            }
            if let Err(e) = imported.save() {
                eprintln!("State import failed: {}", e);
                std::process::exit(1);
            }
            print_line(&format!("State imported from {} ({} sessions)", path, imported.sessions.len()));
            return;
        }
        Mode::Watch => {
            let st = State::load();
            let interval = st.tick_interval_s.clamp(5, 300) as u64;
            print_line(&format!("Watching {} sessions, tick every {}s (Ctrl+C to stop)", st.sessions.len(), interval));
            loop {
                let current = State::load();
                let out = run_tick(current, true);
                print_line(&format!("{}", out.summary));
                for (sid, view) in out.views {
                    print_line(&format!("  {} {} {}", short_id(&sid), status_text(&view.status), view.reason));
                }
                std::thread::sleep(std::time::Duration::from_secs(interval));
            }
        }
        Mode::Pause(sid) | Mode::Resume(sid) => {
            let mut st = State::load();
            let enable = resume_mode;
            match st.sessions.iter_mut().find(|s| s.id == sid) {
                Some(s) => {
                    s.enabled = enable;
                    let _ = st.save();
                    print_line(&format!("{} {}", sid, if enable { "resumed" } else { "paused" }));
                }
                None => {
                    eprintln!("Session {} is not in the watch list", sid);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::SetPrompt(sid, text) => {
            let mut st = State::load();
            match st.sessions.iter_mut().find(|s| s.id == sid) {
                Some(s) => {
                    s.prompt = if text.trim().is_empty() { None } else { Some(text.trim().to_string()) };
                    let _ = st.save();
                    print_line(&format!("Prompt set for {}", sid));
                }
                None => {
                    eprintln!("Session {} is not in the watch list", sid);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::ExportAudit => {
            match write_audit() {
                Ok(path) => println!("Audit written to {}", path.display()),
                Err(e) => {
                    eprintln!("Audit export failed: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::Kick(sid) => {
            let st = State::load();
            let session = match st.sessions.iter().find(|s| s.id == sid) {
                Some(s) => s.clone(),
                None => {
                    eprintln!("Session {} is not in the watch list", sid);
                    std::process::exit(1);
                }
            };
            let prompt = session.prompt.clone().unwrap_or_else(|| st.default_prompt.clone());
            let prompt = prompt.trim();
            let prompt = if prompt.is_empty() { "continue" } else { prompt };
            if Config::dry_run() {
                print_line(&format!("Would kick {} with: {}", sid, prompt));
                return;
            }
            match send_nudge(&Config::api_base(), &Config::password(), &sid, prompt) {
                Ok(()) => {
                    let mut st = State::load();
                    let now = now_ms();
                    let ts = open_db().map(|c| last_idle_ts(&c, &sid)).unwrap_or(now);
                    if let Some(x) = st.sessions.iter_mut().find(|x| x.id == sid) {
                        x.handled = ts;
                        x.prompted = now;
                        x.last_nudge = now;
                        x.nudges += 1;
                        x.idle_streak = 0;
                        x.last_action = format!("kicked {}", session_keeper_rust::config::format_log_time(now / 1000));
                    }
                    let _ = st.save();
                    append_history(&sid, "nudge", prompt);
                    log_line(&format!("{}: kicked from CLI", sid));
                    print_line(&format!("Kicked {}", sid));
                }
                Err(e) => {
                    eprintln!("Kick failed: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }
        Mode::Models(filter) => {
            let models = match fetch_models_result(&Config::api_base(), &Config::password()) {
                Ok(models) => models,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let mut shown = 0;
            for m in models.iter().filter(|m| match &filter {
                Some(f) => m.id.contains(f) || m.name.contains(f),
                None => true,
            }) {
                print_line(&format!("{} | {} | {}", m.id, m.provider, m.name));
                shown += 1;
                if shown >= 30 { break; }
            }
            print_line(&format!("Total available: {} models (showing {})", models.len(), shown));
            return;
        }
        Mode::SetModel(sid, id, provider) => {
            if !State::load().sessions.iter().any(|s| s.id == sid) {
                eprintln!("Session {} is not in the watch list", sid);
                std::process::exit(1);
            }
            match switch_model(&Config::api_base(), &Config::password(), &sid, &id, &provider) {
                Ok(()) => {
                    log_line(&format!("{}: model switched to {} ({})", sid, id, provider));
                    print_line(&format!("Model switched for {}", sid));
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

    if matches!(opts.mode, Mode::Gui) {
        if std::env::var_os("DISPLAY").is_none() {
            eprintln!("Session Keeper GUI needs a display. Set DISPLAY or use --watch in a terminal.");
            return;
        }
        let notify_ok = std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v notify-send >/dev/null 2>&1")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !notify_ok {
            log_line("notify-send not found; desktop alerts will not appear");
        }
    }

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

    let state = Rc::new(RefCell::new(boot_state));
    let views: Rc<RefCell<HashMap<String, SessionView>>> = Rc::new(RefCell::new(HashMap::new()));
    let filter: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let attention: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let busy = Rc::new(Cell::new(false));
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let (tx, rx) = channel::<TickOutcome>();
    let (model_tx, model_rx) = channel::<(String, Result<Vec<ModelInfo>, String>)>();
    let _ = MODEL_TX.set(model_tx.clone());
    let (switch_tx, switch_rx) = channel::<(String, Result<(), String>)>();
    let (kick_tx, kick_rx) = channel::<(String, i64, Result<(), String>)>();
    let (add_tx, add_rx) = channel::<(String, bool)>();
    let (wake_tx, wake_rx) = channel::<()>();
    let event_stop = Arc::new(AtomicBool::new(false));
    let event_connected = Arc::new(AtomicBool::new(false));
    let _ = ADD_TX.set(add_tx.clone());
    let (session_tx, session_rx) = channel::<Result<Vec<SessionInfo>, String>>();
    let _ = SESSION_TX.set(session_tx.clone());
    let _ = KICK_TX.set(kick_tx.clone());
    let model_cache: Rc<RefCell<HashMap<String, Vec<ModelInfo>>>> = Rc::new(RefCell::new(HashMap::new()));

    let filter_entry = gtk::Entry::new();
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("Session Keeper");
    window.set_default_size(960, 680);
    window.set_size_request(760, 520);
    let stop_on_close = event_stop.clone();
    window.connect_destroy(move |_| {
        stop_on_close.store(true, Ordering::Relaxed);
        gtk::main_quit();
    });
    let st_key = state.clone();
    let tx_key = tx.clone();
    let busy_key = busy.clone();
    let filter_key = filter_entry.clone();
    window.connect_key_press_event(move |_, key| {
        use gtk::gdk::keys::constants;
        if key.keyval() == constants::F5 {
            launch_tick(&st_key, &tx_key, &busy_key);
            return glib::Propagation::Stop;
        }
        let ctrl = key.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
        if ctrl && key.keyval() == constants::q {
            gtk::main_quit();
            return glib::Propagation::Stop;
        }
        if ctrl && key.keyval() == constants::f {
            filter_key.grab_focus();
            return glib::Propagation::Stop;
        }
        if ctrl && key.keyval() == constants::e {
            if let Err(e) = write_audit() {
                log_line(&format!("audit export failed: {}", e));
            }
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
    let event_label = gtk::Label::new(Some("Events: connecting..."));
    event_label.style_context().add_class("note");
    event_label.set_xalign(0.0);
    outer.add(&event_label);

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("Session id (ses_...)"));
    entry.set_hexpand(true);
    controls.add(&entry);

    filter_entry.set_placeholder_text(Some("Filter sessions..."));
    filter_entry.set_width_chars(18);
    controls.add(&filter_entry);
    let attention_cb = gtk::CheckButton::with_label("Attention only");
    controls.add(&attention_cb);

    let pause_toggle = gtk::ToggleButton::with_label(if state.borrow().enabled { "Pause all" } else { "Resume all" });
    pause_toggle.style_context().add_class("destructive-action");
    let st_pause = state.clone();
    let summary_pause = summary.clone();
    pause_toggle.connect_toggled(move |b| {
        let enabled = !b.is_active();
        st_pause.borrow_mut().enabled = enabled;
        let _ = st_pause.borrow().save();
        b.set_label(if enabled { "Pause all" } else { "Resume all" });
        summary_pause.set_label(if enabled { "Watching enabled" } else { "All automatic nudges paused" });
    });
    controls.add(&pause_toggle);

    let add = icon_button("Add session", "list-add-symbolic");
    let st_add = state.clone();
    let list_add = Rc::new(RefCell::new(gtk::Box::new(gtk::Orientation::Vertical, 2)));
    let views_add = views.clone();
    let filter_add = filter.clone();
    let summary_add = summary.clone();
    let entry_add = entry.clone();
    add.connect_clicked(clone!(@strong st_add, @strong list_add, @strong views_add, @strong filter_add, @strong summary_add, @strong entry_add => move |_| {
        let sid = entry_add.text().trim().to_string();
        if !sid.starts_with("ses_") {
            summary_add.set_label("Session id must start with ses_");
            return;
        }
        if st_add.borrow().sessions.iter().any(|s| s.id == sid) {
            summary_add.set_label("Session is already watched");
            return;
        }
        let tx = match ADD_TX.get() { Some(t) => t.clone(), None => return };
        let check_sid = sid.clone();
        std::thread::spawn(move || {
            let exists = open_db().map(|c| session_exists(&c, &check_sid)).unwrap_or(false);
            let _ = tx.send((check_sid, exists));
        });
        entry_add.set_text("");
        summary_add.set_label("Checking session id...");
    }));
    controls.add(&add);

    let discover = icon_button("Discover", "system-search-symbolic");
    discover.connect_clicked(|_| {
        let tx = match SESSION_TX.get() { Some(t) => t.clone(), None => return };
        let api = Config::api_base();
        let pw = Config::password();
        std::thread::spawn(move || {
            let sessions = fetch_sessions_result(&api, &pw);
            let _ = tx.send(sessions);
        });
    });
    controls.add(&discover);

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
    let sort_cb = gtk::CheckButton::with_label("Sort sessions by name instead of priority");
    sort_cb.set_active(state.borrow().sort_by_name);
    settings_box.add(&sort_cb);

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
    row3.add(&gtk::Label::new(Some("Alert cooldown (s):")));
    let cooldown_spin = gtk::SpinButton::new(
        Some(&gtk::Adjustment::new(state.borrow().alert_cooldown_s as f64, 30.0, 3600.0, 30.0, 300.0, 0.0)),
        1.0, 0,
    );
    row3.add(&cooldown_spin);
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

    let test_notify = icon_button("Test notification", "dialog-information-symbolic");
    let notify_result = gtk::Label::new(Some(""));
    notify_result.style_context().add_class("note");
    notify_result.set_xalign(0.0);
    let notify_result_btn = notify_result.clone();
    test_notify.connect_clicked(move |_| {
        let sent = notify("Session Keeper", "Test notification from Session Keeper");
        notify_result_btn.set_label(if sent {
            "Test notification sent"
        } else if !Config::notifications() {
            "Notifications disabled in settings"
        } else {
            "Notification failed: notify-send unavailable (see log)"
        });
    });
    settings_box.add(&test_notify);
    settings_box.add(&notify_result);

    let save_settings = icon_button("Save settings", "document-save-symbolic");
    save_settings.style_context().add_class("suggested-action");
    let st_settings = state.clone();
    let tx_settings = tx.clone();
    let busy_settings = busy.clone();
    let timer_settings = timer.clone();
    save_settings.connect_clicked(move |_| {
        let p = def_prompt.text().trim().to_string();
        let default_prompt = if p.is_empty() { "continue, dont stop".to_string() } else { p };
        let dry = dry_cb.is_active();
        let notify = notify_cb.is_active();
        {
            let mut st = st_settings.borrow_mut();
            st.default_prompt = default_prompt;
            st.min_gap = min_gap.value() as i64;
            st.max_nudges_per_hour = cap.value() as i64;
            st.away_mode = away.is_active();
            st.global_nudge_cap = global_cap.value() as i64;
            st.enabled = enabled_cb.is_active();
            st.persist_dry_run = dry;
            st.persist_notifications = notify;
            st.sort_by_name = sort_cb.is_active();
            st.alert_cooldown_s = cooldown_spin.value() as i64;
            st.tick_interval_s = interval_spin.value() as i64;
            let _ = st.save();
        }
        std::env::set_var("SESSION_KEEPER_DRY_RUN", if dry { "1" } else { "0" });
        std::env::set_var("SESSION_KEEPER_NOTIFY", if notify { "1" } else { "0" });
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
            .and_then(|s| State::from_json(&s).ok())
        {
            Some(new_state) => {
                let backup = session_keeper_rust::state::backup_state();
                *st_import.borrow_mut() = new_state;
                let _ = st_import.borrow().save();
                summary_import.set_label(&format!("Imported from {}{}", path.display(),
                    backup.map(|b| format!(" (backup: {})", b.display())).unwrap_or_default()));
                refresh(&st_import, &list_import, &views_import, &filter_import, &attention_import);
            }
            None => summary_import.set_label("Import failed: no valid export file"),
        }
    });
    io_row.add(&import_btn);
    let reload_btn = icon_button("Reload from disk", "view-refresh-symbolic");
    let st_reload = state.clone();
    let list_reload = list.clone();
    let views_reload = views.clone();
    let filter_reload = filter.clone();
    let attention_reload = attention.clone();
    let summary_reload = summary.clone();
    reload_btn.connect_clicked(move |_| {
        *st_reload.borrow_mut() = State::load();
        refresh(&st_reload, &list_reload, &views_reload, &filter_reload, &attention_reload);
        summary_reload.set_label("Settings reloaded from disk");
    });
    io_row.add(&reload_btn);
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
        if let Err(e) = write_audit() {
            log_line(&format!("audit export failed: {}", e));
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

    let help = gtk::Expander::new(Some("Help: shortcuts and terminal commands"));
    let help_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let help_text = gtk::Label::new(Some(
        "Shortcuts: F5 check now, Ctrl+F focus filter, Ctrl+E export audit, Ctrl+Q quit.\
Terminal: --check-now, --status [--json], --watch, --kick <sid>, --models, --set-model, --pause/--resume <sid>, --set-prompt <sid> <text>, --set-setting <key> <value>, --export-state/--import-state <path>, --export-audit, --dry-run, --help.",
    ));
    help_text.set_xalign(0.0);
    help_text.set_line_wrap(true);
    help_text.style_context().add_class("session-meta");
    help_box.add(&help_text);
    help.add(&help_box);
    outer.add(&help);

    // Apply worker results on the UI thread.
    let rx_cell = Rc::new(RefCell::new(Some(rx)));
    let st_ui = state.clone();
    let views_ui = views.clone();
    let filter_ui = filter.clone();
    let attention_ui = attention.clone();
    let summary_ui = summary.clone();
    let pause_ui = pause_toggle.clone();
    let title_ui = window.clone();
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
                    if let Some(err) = outcome.error {
                        // A worker panic must not leave Check now disabled forever.
                        // Keep the last good views and state; just surface the error.
                        summary_ui.set_label(&format!("Watchdog error: {} (Check now to retry)", err));
                        busy_ui.set(false);
                    } else {
                        {
                            // Disk is authoritative for settings and watch-list edits made by
                            // the CLI or another process. Only tick-owned fields come from
                            // the worker snapshot, so an external pause cannot be undone by
                            // a tick finishing a moment later.
                            let mut merged = State::load();
                            apply_tick_fields(&mut merged, &outcome.updates, outcome.globals);
                            let _ = merged.save();
                            pause_ui.set_label(if merged.enabled { "Pause all" } else { "Resume all" });
                            *st_ui.borrow_mut() = merged;
                        }
                        *views_ui.borrow_mut() = outcome.views;
                        let watched = st_ui.borrow().sessions.len();
                        let attention = views_ui.borrow().values().filter(|v| needs_attention(&v.status)).count();
                        let prefix = if Config::dry_run() { "DRY RUN | " } else { "" };
                        let cap_text = if outcome.globals.cap_remaining < 0 {
                            String::new()
                        } else {
                            format!("  |  global cap {} left", outcome.globals.cap_remaining)
                        };
                        summary_ui.set_label(&format!("{}{}  |  {} watched, {} need attention{}",
                            prefix, outcome.summary, watched, attention, cap_text));
                        title_ui.set_title(&format!("Session Keeper - {} need attention", attention));
                        history_ui.set_label(&tail_file(&Config::history_path(), 25));
                        log_ui.set_label(&tail_file(&Config::log_path(), 25));
                        digest_ui.set_label(&today_digest());
                        refresh(&st_ui, &list_ui, &views_ui, &filter_ui, &attention_ui);
                        busy_ui.set(false);
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    log_line("Watchdog error: worker channel disconnected");
                    summary_ui.set_label("Watchdog error: worker stopped (restart app)");
                    busy_ui.set(false);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let models_state = model_cache.clone();
    let views_model = views.clone();
    let tx_switch = switch_tx.clone();
    let summary_model = summary.clone();
    let model_rx_cell = Rc::new(RefCell::new(Some(model_rx)));
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = model_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, result)) = r.try_recv() {
                let models = match result {
                    Ok(models) => models,
                    Err(e) => {
                        summary_model.set_label(&format!("Models failed: {}", e));
                        return glib::ControlFlow::Continue;
                    }
                };
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
                if !models.is_empty() {
                    let current = views_model.borrow().get(&sid).map(|v| v.model.clone()).unwrap_or_default();
                    let active = models.iter().find(|m| !current.is_empty() && m.id == current)
                        .map(|m| format!("{}|{}", m.id, m.provider))
                        .or_else(|| Some(format!("{}|{}", models[0].id, models[0].provider)));
                    combo.set_active_id(active.as_deref());
                } else {
                    dialog.content_area().add(&gtk::Label::new(Some("No models returned by OpenCode.")));
                }
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

    let add_rx_cell = Rc::new(RefCell::new(Some(add_rx)));
    let st_add_result = state.clone();
    let list_add_result = list.clone();
    let views_add_result = views.clone();
    let filter_add_result = filter.clone();
    let attention_add_result = attention.clone();
    let summary_add_result = summary.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = add_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, exists)) = r.try_recv() {
                if !exists {
                    summary_add_result.set_label("Session id not found in OpenCode database");
                } else if !st_add_result.borrow().sessions.iter().any(|s| s.id == sid) {
                    st_add_result.borrow_mut().sessions.push(Session {
                        id: sid.clone(),
                        enabled: true,
                        prompt: None,
                        handled: 0,
                        prompted: 0,
                        last_nudge: 0,
                        nudges: 0,
                        last_action: "added".to_string(),
                        ..Default::default()
                    });
                    let _ = st_add_result.borrow().save();
                    summary_add_result.set_label(&format!("Added {}", session_keeper_rust::ui::short_id(&sid)));
                    refresh(&st_add_result, &list_add_result, &views_add_result, &filter_add_result, &attention_add_result);
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let session_rx_cell = Rc::new(RefCell::new(Some(session_rx)));
    let st_discover = state.clone();
    let list_discover = list.clone();
    let views_discover = views.clone();
    let filter_discover = filter.clone();
    let attention_discover = attention.clone();
    let summary_discover = summary.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = session_rx_cell.borrow_mut().as_mut() {
            if let Ok(result) = r.try_recv() {
                let found = match result {
                    Ok(found) => found,
                    Err(e) => {
                        summary_discover.set_label(&format!("Discover failed: {}", e));
                        return glib::ControlFlow::Continue;
                    }
                };
                if found.is_empty() {
                    summary_discover.set_label("No sessions returned by OpenCode");
                } else {
                    let dialog = gtk::Dialog::with_buttons(
                        Some("Add a session"),
                        None::<&gtk::Window>,
                        gtk::DialogFlags::MODAL,
                        &[("Cancel", gtk::ResponseType::Cancel), ("Add", gtk::ResponseType::Ok)],
                    );
                    let combo = gtk::ComboBoxText::new();
                    for s in &found {
                        let label = if s.title.is_empty() { s.id.clone() } else { format!("{} - {}", s.title, s.id) };
                        combo.append(Some(&s.id), &label);
                    }
                    combo.set_active(Some(0));
                    dialog.content_area().add(&combo);
                    dialog.show_all();
                    let st = st_discover.clone();
                    let list = list_discover.clone();
                    let views = views_discover.clone();
                    let filter = filter_discover.clone();
                    let attention = attention_discover.clone();
                    let summary = summary_discover.clone();
                    dialog.connect_response(move |dlg, resp| {
                        if resp == gtk::ResponseType::Ok {
                            if let Some(id) = combo.active_id() {
                                let sid = id.to_string();
                                if session_keeper_rust::state::is_valid_session_id(&sid)
                                    && !st.borrow().sessions.iter().any(|s| s.id == sid)
                                {
                                    st.borrow_mut().sessions.push(Session {
                                        id: sid.clone(),
                                        enabled: true,
                                        prompt: None,
                                        handled: 0,
                                        prompted: 0,
                                        last_nudge: 0,
                                        nudges: 0,
                                        last_action: "discovered".to_string(),
                                        ..Default::default()
                                    });
                                    let _ = st.borrow().save();
                                    summary.set_label(&format!("Added {}", session_keeper_rust::ui::short_id(&sid)));
                                    refresh(&st, &list, &views, &filter, &attention);
                                }
                            }
                        }
                        dlg.close();
                    });
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let kick_rx_cell = Rc::new(RefCell::new(Some(kick_rx)));
    let summary_kick = summary.clone();
    let st_kick_result = state.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = kick_rx_cell.borrow_mut().as_mut() {
            if let Ok((sid, ts, result)) = r.try_recv() {
                match result {
                    Ok(()) if Config::dry_run() => summary_kick.set_label(&format!("Dry-run kick for {}", short_id(&sid))),
                    Ok(()) => {
                        let mut st = st_kick_result.borrow_mut();
                        let now = now_ms();
                        if let Some(x) = st.sessions.iter_mut().find(|x| x.id == sid) {
                            x.handled = ts;
                            x.prompted = now;
                            x.last_nudge = now;
                            x.nudges += 1;
                            x.idle_streak = 0;
                            x.last_action = format!("kicked {}", session_keeper_rust::config::format_log_time(now / 1000));
                        }
                        let _ = st.save();
                        drop(st);
                        summary_kick.set_label(&format!("Kicked {}", short_id(&sid)));
                    }
                    Err(e) => summary_kick.set_label(&format!("Kick failed: {}", e)),
                }
            }
        }
        glib::ControlFlow::Continue
    });

    let st_watch = state.clone();
    let pause_watch = pause_toggle.clone();
    let list_watch = list.clone();
    let views_watch = views.clone();
    let filter_watch = filter.clone();
    let attention_watch = attention.clone();
    let summary_watch = summary.clone();
    let tx_watch = tx.clone();
    let timer_watch = timer.clone();
    let busy_watch = busy.clone();
    glib::timeout_add_seconds_local(5, move || {
        let disk = State::load();
        let (changed, old_interval) = {
            let mem = st_watch.borrow();
            (states_differ(&mem, &disk), mem.tick_interval_s)
        };
        if changed {
            *st_watch.borrow_mut() = disk.clone();
            if old_interval != disk.tick_interval_s {
                arm_timer(&st_watch, &tx_watch, &busy_watch, &timer_watch);
            }
            pause_watch.set_label(if disk.enabled { "Pause all" } else { "Resume all" });
            // A settings change made by CLI or another process must reach the
            // watchdog workers, not just the GUI's in-memory state.
            std::env::set_var(
                "SESSION_KEEPER_DRY_RUN",
                if disk.persist_dry_run { "1" } else { "0" },
            );
            std::env::set_var(
                "SESSION_KEEPER_NOTIFY",
                if disk.persist_notifications { "1" } else { "0" },
            );
            refresh(&st_watch, &list_watch, &views_watch, &filter_watch, &attention_watch);
            summary_watch.set_label("State reloaded from disk");
            log_line("state reloaded from disk (external edit)");
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

    let wake_rx_cell = Rc::new(RefCell::new(Some(wake_rx)));
    let st_wake = state.clone();
    let tx_wake = tx.clone();
    let busy_wake = busy.clone();
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if let Some(r) = wake_rx_cell.borrow_mut().as_mut() {
            while r.try_recv().is_ok() {
                launch_tick(&st_wake, &tx_wake, &busy_wake);
            }
        }
        glib::ControlFlow::Continue
    });

    let event_api = Config::api_base();
    let event_pw = Config::password();
    let event_stop_thread = event_stop.clone();
    let event_state = event_connected.clone();
    std::thread::spawn(move || {
        watch_events(&event_api, &event_pw, wake_tx, event_stop_thread, event_state);
    });

    let event_label_timer = event_label.clone();
    let event_indicator = event_connected.clone();
    glib::timeout_add_seconds_local(5, move || {
        event_label_timer.set_label(if event_indicator.load(Ordering::Relaxed) {
            "Events: connected (wakeups active)"
        } else {
            "Events: offline (30s polling fallback active)"
        });
        glib::ControlFlow::Continue
    });

    arm_timer(&state, &tx, &busy, &timer);
    refresh(&state, &list, &views, &filter, &attention);
    window.show_all();
    launch_tick(&state, &tx, &busy);
    gtk::main();
}
