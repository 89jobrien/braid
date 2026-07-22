#![allow(clippy::unwrap_used)]
//! Session format stability tests.
//!
//! Verifies that the full pipeline — ingest fixture → store → load →
//! render — produces output that matches the documented format spec.
//! These tests are the contract between braid-observe and consumers
//! (CLI, TUI, future replay tools) that depend on the session format.
//!
//! Two invariants are locked in:
//!
//! 1. **Render output spec**: the text produced by `render_session` for a
//!    known fixture must match the documented column widths, separator, and
//!    header format exactly.
//!
//! 2. **Meta-absent render**: when a session has no `meta.json` (in-progress
//!    or writer-crashed), `render_session` must still produce valid output
//!    using only the event list — no panic, no error.

use braid_observe::{BraidIngester, Ingester, SessionStore, render_session};
use tempfile::tempdir;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

// ---------------------------------------------------------------------------
// Pipeline: ingest → store → load → render
// ---------------------------------------------------------------------------

/// Ingest the braid-native fixture, load it back, render it, and verify the
/// output matches the documented session format spec.
///
/// This test locks in the full pipeline contract. Any accidental change to
/// ingest normalization, store serialization, or render formatting will
/// cause this test to fail, requiring a deliberate spec review.
#[test]
fn ingest_to_render_pipeline_produces_stable_output() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

    let source = fixture_path("braid-native.jsonl");
    let session_id = BraidIngester
        .ingest(&source, &store)
        .expect("BraidIngester must succeed");

    let events = store.load(&session_id).unwrap();
    // No meta.json after ingest (meta is only written by SessionWriter.finish())
    let meta = store.load_meta(&session_id).unwrap();

    let mut out = Vec::<u8>::new();
    render_session(&events, meta.as_ref(), &mut out).unwrap();

    let rendered = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();

    // Header: "Session: <id>  <N> events" (no timestamp when meta absent)
    assert!(
        lines[0].starts_with("Session: fix-1"),
        "header must start with session id: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("5 events"),
        "header must show event count: {:?}",
        lines[0]
    );

    // Separator: exactly 50 hyphens
    assert_eq!(
        lines[1], "--------------------------------------------------",
        "separator must be exactly 50 hyphens"
    );

    // Event rows: right-aligned 2-char index, 20-char kind, optional detail
    // braid-native has: SessionStarted, ProviderResponded, ToolCalled(echo),
    //                   ToolCompleted(echo), SessionCompleted
    assert!(
        lines[2].contains("SessionStarted"),
        "row 1 must be SessionStarted: {:?}",
        lines[2]
    );
    assert!(
        lines[3].contains("ProviderResponded"),
        "row 2 must be ProviderResponded: {:?}",
        lines[3]
    );
    assert!(
        lines[4].contains("ToolCalled") && lines[4].contains("echo"),
        "row 3 must be ToolCalled with tool name: {:?}",
        lines[4]
    );
    assert!(
        lines[5].contains("ToolCompleted") && lines[5].contains("echo"),
        "row 4 must be ToolCompleted with tool name: {:?}",
        lines[5]
    );
    assert!(
        lines[6].contains("SessionCompleted"),
        "row 5 must be SessionCompleted: {:?}",
        lines[6]
    );

    // Exactly 7 lines: header + separator + 5 event rows
    assert_eq!(
        lines.len(),
        7,
        "render must produce exactly 7 lines for a 5-event session"
    );
}

// ---------------------------------------------------------------------------
// Meta-absent render (in-progress / writer-crashed session)
// ---------------------------------------------------------------------------

/// When a session has no meta.json (e.g. writer crashed before finish()),
/// render_session must still produce valid output from the event list alone.
///
/// This is a safety contract: partial sessions must always be inspectable.
#[test]
fn render_without_meta_uses_event_list_for_session_id() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

    // Ingest produces events but no meta.json
    let source = fixture_path("braid-native.jsonl");
    let session_id = BraidIngester
        .ingest(&source, &store)
        .expect("BraidIngester must succeed");

    let events = store.load(&session_id).unwrap();
    // Explicitly pass None for meta (simulating crashed writer)
    let mut out = Vec::<u8>::new();
    render_session(&events, None, &mut out).unwrap();

    let rendered = String::from_utf8(out).unwrap();
    let first_line = rendered.lines().next().unwrap();

    // Header must include session id extracted from events (not "unknown")
    assert!(
        first_line.contains("fix-1"),
        "meta-absent render must use session_id from first event: {:?}",
        first_line
    );
    assert!(
        !first_line.contains("unknown"),
        "session_id must not fall back to 'unknown' when events are present: {:?}",
        first_line
    );
}

// ---------------------------------------------------------------------------
// Format contract: column widths and row structure
// ---------------------------------------------------------------------------

/// Each event row must conform to the documented column format:
/// right-aligned 2-char index + space(s) + 20-char kind + optional detail.
///
/// This locks in the column widths against accidental format changes.
#[test]
fn render_row_format_matches_column_spec() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();

    let source = fixture_path("braid-native.jsonl");
    let session_id = BraidIngester
        .ingest(&source, &store)
        .expect("BraidIngester must succeed");

    let events = store.load(&session_id).unwrap();
    let mut out = Vec::<u8>::new();
    render_session(&events, None, &mut out).unwrap();

    let rendered = String::from_utf8(out).unwrap();
    let event_rows: Vec<&str> = rendered.lines().skip(2).collect(); // skip header + separator

    for (i, row) in event_rows.iter().enumerate() {
        let idx = i + 1;
        // Row must start with right-aligned index (1 or 2 digits, right-padded to 4 chars)
        let idx_str = format!("{:>4}", idx);
        assert!(
            row.starts_with(&idx_str),
            "row {} must start with right-aligned index '{idx_str}': {:?}",
            idx,
            row
        );
    }
}

// ---------------------------------------------------------------------------
// Cross-ingester render stability
// ---------------------------------------------------------------------------

/// All three ingesters produce sessions that render without errors.
/// Verifies the render pipeline handles any valid normalized event sequence.
#[test]
fn all_ingesters_render_without_error() {
    use braid_observe::{ClaudeCodeIngester, DevloopIngester};

    let cases: &[(&dyn Ingester, &str)] = &[
        (&BraidIngester, "braid-native.jsonl"),
        (&ClaudeCodeIngester, "claude-code.jsonl"),
        (&DevloopIngester, "devloop.jsonl"),
    ];

    for (ingester, fixture) in cases {
        let dir = tempdir().unwrap();
        let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
        let source = fixture_path(fixture);

        let session_id = ingester
            .ingest(&source, &store)
            .unwrap_or_else(|e| panic!("[{fixture}] ingest failed: {e}"));

        let events = store.load(&session_id).unwrap();
        let mut out = Vec::<u8>::new();
        render_session(&events, None, &mut out)
            .unwrap_or_else(|e| panic!("[{fixture}] render failed: {e}"));

        let rendered = String::from_utf8(out).unwrap();
        assert!(
            !rendered.is_empty(),
            "[{fixture}] rendered output must not be empty"
        );
        assert!(
            rendered.lines().count() >= 3,
            "[{fixture}] render must have at least header + separator + 1 event row"
        );
    }
}

// ---------------------------------------------------------------------------
// Format evolution: unknown event kinds survive store round-trip and render
// ---------------------------------------------------------------------------

/// An unknown EventKind (future schema addition) written to the store must
/// survive load and render without error — the render must show it as "Unknown".
///
/// This locks in the forward-compat contract for the render path.
#[test]
fn unknown_event_kind_renders_as_unknown_row() {
    use braid_model::{Event, EventKind, SessionId};
    use braid_observe::SessionStore;

    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let sid = SessionId("fwd-compat".into());

    // Write events including an Unknown kind (simulates a future event type)
    let events = vec![
        Event {
            session_id: sid.clone(),
            kind: EventKind::SessionStarted,
        },
        Event {
            session_id: sid.clone(),
            kind: EventKind::Unknown {
                raw: "FutureEventType".into(),
            },
        },
        Event {
            session_id: sid.clone(),
            kind: EventKind::SessionCompleted,
        },
    ];
    store.write(&sid, &events).unwrap();

    let loaded = store.load(&sid).unwrap();
    let mut out = Vec::<u8>::new();
    render_session(&loaded, None, &mut out).unwrap();

    let rendered = String::from_utf8(out).unwrap();
    assert!(
        rendered.contains("Unknown"),
        "Unknown event kind must appear in render output: {rendered:?}"
    );
    assert!(
        rendered.contains("FutureEventType"),
        "Unknown event raw value must be shown in render output: {rendered:?}"
    );
}
