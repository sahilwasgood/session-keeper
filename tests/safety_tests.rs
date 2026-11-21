use session_keeper_rust::safety::{fingerprint, should_nudge_after_debounce};

#[test]
fn fingerprint_is_stable_and_distinct() {
    assert_eq!(fingerprint("same output"), fingerprint("same output"));
    assert_ne!(fingerprint("same output"), fingerprint("other output"));
}

#[test]
fn debounce_waits_for_second_idle_check() {
    assert!(!should_nudge_after_debounce(0));
    assert!(!should_nudge_after_debounce(1));
    assert!(should_nudge_after_debounce(2));
}

#[test]
fn repeating_output_detects_aa_and_aba_loops() {
    use session_keeper_rust::safety::is_repeating_output;
    assert!(is_repeating_output("A", Some("A"), Some("B")));
    assert!(is_repeating_output("A", Some("B"), Some("A")));
    assert!(!is_repeating_output("A", Some("B"), Some("C")));
    assert!(!is_repeating_output("A", None, None));
}
