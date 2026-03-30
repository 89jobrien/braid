//! Ingester conformance suite.
//!
//! Validates that BraidIngester, DevloopIngester, and ClaudeCodeIngester all
//! produce normalized session events that satisfy the same structural invariants
//! for the same logical session shape. Each ingester reads a different source
//! format but must converge on the same normalized `Event` model.

use braid_model::EventKind;
use braid_observe::{BraidIngester, ClaudeCodeIngester, DevloopIngester, Ingester, SessionStore};
use tempfile::tempdir;

fn fixture_path(name: &str) -> std::path::PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest)
        .join("fixtures")
        .join(name)
}

/// Common invariants every conforming ingester must satisfy.
fn assert_session_invariants(
    label: &str,
    store: &SessionStore,
    session_id: &braid_model::SessionId,
) {
    let events = store
        .load(session_id)
        .unwrap_or_else(|e| panic!("[{label}] store.load failed: {e}"));

    // 1. Non-empty event list
    assert!(
        !events.is_empty(),
        "[{label}] session must contain at least one event"
    );

    // 2. Session ID is set (non-empty string)
    assert!(
        !session_id.0.is_empty(),
        "[{label}] session_id must be non-empty"
    );

    // 3. All events carry the same session_id as the returned one
    for (i, event) in events.iter().enumerate() {
        assert_eq!(
            &event.session_id, session_id,
            "[{label}] event[{i}].session_id mismatch"
        );
    }

    // 4. First event is SessionStarted
    assert_eq!(
        events[0].kind,
        EventKind::SessionStarted,
        "[{label}] first event must be SessionStarted"
    );

    // 5. Last event is SessionCompleted
    assert_eq!(
        events.last().unwrap().kind,
        EventKind::SessionCompleted,
        "[{label}] last event must be SessionCompleted"
    );

    // 6. At least one ProviderResponded event
    let has_provider = events
        .iter()
        .any(|e| e.kind == EventKind::ProviderResponded);
    assert!(
        has_provider,
        "[{label}] session must contain at least one ProviderResponded event"
    );

    // 7. No Unknown events — all source events must map to a recognized kind
    for (i, event) in events.iter().enumerate() {
        assert!(
            !matches!(&event.kind, EventKind::Unknown { .. }),
            "[{label}] event[{i}] must not be Unknown; ingesters must normalize all recognized kinds"
        );
    }
}

#[test]
fn braid_ingester_conformance() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let source = fixture_path("braid-native.jsonl");

    let id = BraidIngester
        .ingest(&source, &store)
        .expect("BraidIngester must succeed on braid-native.jsonl");

    assert_session_invariants("BraidIngester", &store, &id);
}

#[test]
fn claude_code_ingester_conformance() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let source = fixture_path("claude-code.jsonl");

    let id = ClaudeCodeIngester
        .ingest(&source, &store)
        .expect("ClaudeCodeIngester must succeed on claude-code.jsonl");

    assert_session_invariants("ClaudeCodeIngester", &store, &id);
}

#[test]
fn devloop_ingester_conformance() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let source = fixture_path("devloop.jsonl");

    let id = DevloopIngester
        .ingest(&source, &store)
        .expect("DevloopIngester must succeed on devloop.jsonl");

    assert_session_invariants("DevloopIngester", &store, &id);
}

/// All three ingesters must produce sessions whose event sequences share
/// the same structural shape: [SessionStarted, ..., SessionCompleted].
/// This test runs all three and cross-checks the shape property holds
/// for each independently, confirming convergence on the normalized model.
#[test]
fn all_ingesters_produce_consistent_event_shape() {
    struct Case {
        label: &'static str,
        fixture: &'static str,
    }

    let cases: &[(&dyn Ingester, Case)] = &[
        (
            &BraidIngester,
            Case {
                label: "BraidIngester",
                fixture: "braid-native.jsonl",
            },
        ),
        (
            &ClaudeCodeIngester,
            Case {
                label: "ClaudeCodeIngester",
                fixture: "claude-code.jsonl",
            },
        ),
        (
            &DevloopIngester,
            Case {
                label: "DevloopIngester",
                fixture: "devloop.jsonl",
            },
        ),
    ];

    for (ingester, case) in cases {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path(case.fixture);

        let id = ingester
            .ingest(&source, &store)
            .unwrap_or_else(|e| panic!("[{}] ingest failed: {e}", case.label));

        let events = store.load(&id).unwrap();

        assert_eq!(
            events.first().map(|e| &e.kind),
            Some(&EventKind::SessionStarted),
            "[{}] first event must be SessionStarted",
            case.label
        );
        assert_eq!(
            events.last().map(|e| &e.kind),
            Some(&EventKind::SessionCompleted),
            "[{}] last event must be SessionCompleted",
            case.label
        );
    }
}
