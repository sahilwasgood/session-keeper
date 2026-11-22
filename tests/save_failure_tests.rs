use session_keeper_rust::state::State;

#[test]
fn save_failure_is_logged_once() {
    let dir = std::env::temp_dir().join(format!("sk-savefail-{}-{}", std::process::id(), 1));
    let _ = std::fs::create_dir_all(&dir);
    // A regular file where a parent directory is expected makes create_dir_all fail.
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let log = dir.join("keeper.log");

    std::env::set_var("SESSION_KEEPER_STATE", blocker.join("state.json"));
    std::env::set_var("SESSION_KEEPER_LOG", &log);

    let result = State::default().save();
    assert!(result.is_err(), "save into a file-as-directory must fail");
    // A second failure must not add another line.
    let _ = State::default().save();

    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let hits = text.matches("State save failed").count();
    assert_eq!(
        hits, 1,
        "expected exactly one save-failure log line, got {}",
        hits
    );

    std::fs::remove_dir_all(&dir).ok();
}
