//! Redaction coverage audit for all persist paths in braid-observe.
//!
//! Design invariant: redact-before-persist.  Every path through which events
//! or transcripts reach disk MUST have redaction applied by the caller before
//! handing data to the store.  These tests document and enforce that invariant.
//!
//! Audit scope:
//!   1. `SessionStore::write()`  — batch persist of a Vec<Event>
//!   2. `SessionWriter::write_event()` — streaming per-event persist
//!   3. `BraidIngester::ingest()` — native JSONL ingest path
//!
//! Each test:
//!   (a) First demonstrates that writing WITHOUT redaction leaves the raw
//!       secret visible in the on-disk JSONL (establishing the risk).
//!   (b) Then demonstrates that applying `RedactionPipeline::redact_event`
//!       before writing prevents the secret from reaching disk.

use braid_model::{Event, EventKind, SessionId};
use braid_observe::store::{SessionStore, SessionWriter};
use braid_redact::{RedactionPipeline, patterns::SecretPatternRule};
use std::io::Read;
use tempfile::tempdir;

// A secret that SecretPatternRule will catch (OpenAI-style key prefix).
const SECRET: &str = "sk-abcdefghijklmnopqrstuvwxyz";
const REDACTED_TOKEN: &str = "[REDACTED:api-key]";

fn secret_tool_event(session_id: &str) -> Event {
    Event {
        session_id: SessionId(session_id.into()),
        kind: EventKind::ToolCalled {
            tool_name: format!("read_file(api_key={SECRET})"),
        },
    }
}

fn read_events_jsonl(store: &SessionStore, id: &SessionId) -> String {
    // Read the raw bytes from disk so we can inspect what was actually written.
    let root = store.root();
    let path = root.join(&id.0).join("events.jsonl");
    let mut f = std::fs::File::open(&path).expect("events.jsonl must exist");
    let mut buf = String::new();
    f.read_to_string(&mut buf).expect("read failed");
    buf
}

// ---------------------------------------------------------------------------
// 1. SessionStore::write() — batch path
// ---------------------------------------------------------------------------

/// Without redaction, a secret in a ToolCalled event survives to disk verbatim.
/// This test documents the risk: callers MUST redact before calling write().
#[test]
fn session_store_write_without_redaction_secret_visible_on_disk() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let id = SessionId("audit-batch-raw".into());

    store
        .write(&id, &[secret_tool_event("audit-batch-raw")])
        .unwrap();

    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        on_disk.contains(SECRET),
        "without redaction the secret is visible on disk — callers must redact first"
    );
}

/// With redaction applied before write(), the secret is absent from disk.
#[test]
fn session_store_write_with_redaction_secret_absent_from_disk() {
    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let id = SessionId("audit-batch-redacted".into());

    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());
    let events: Vec<Event> = vec![secret_tool_event("audit-batch-redacted")]
        .into_iter()
        .map(|e| pipeline.redact_event(&e))
        .collect();

    store.write(&id, &events).unwrap();

    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        !on_disk.contains(SECRET),
        "after redact_event the secret must not appear on disk"
    );
    assert!(
        on_disk.contains(REDACTED_TOKEN),
        "redaction token must be present on disk"
    );
}

// ---------------------------------------------------------------------------
// 2. SessionWriter::write_event() — streaming path
// ---------------------------------------------------------------------------

/// Without redaction, a secret written via SessionWriter is visible on disk.
#[test]
fn session_writer_without_redaction_secret_visible_on_disk() {
    let dir = tempdir().unwrap();
    let id = SessionId("audit-stream-raw".into());

    let mut writer = SessionWriter::open(dir.path(), &id).unwrap();
    writer
        .write_event(&secret_tool_event("audit-stream-raw"))
        .unwrap();
    writer.finish().unwrap();

    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        on_disk.contains(SECRET),
        "without redaction the secret is visible in streamed events"
    );
}

/// With redaction applied before write_event(), the secret is absent from disk.
#[test]
fn session_writer_with_redaction_secret_absent_from_disk() {
    let dir = tempdir().unwrap();
    let id = SessionId("audit-stream-redacted".into());

    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());
    let redacted = pipeline.redact_event(&secret_tool_event("audit-stream-redacted"));

    let mut writer = SessionWriter::open(dir.path(), &id).unwrap();
    writer.write_event(&redacted).unwrap();
    writer.finish().unwrap();

    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        !on_disk.contains(SECRET),
        "after redact_event the secret must not appear in streamed events"
    );
    assert!(
        on_disk.contains(REDACTED_TOKEN),
        "redaction token must be present in streamed events on disk"
    );
}

// ---------------------------------------------------------------------------
// 3. EventSink (buffer → flush) path via EventSink::record + flush
// ---------------------------------------------------------------------------

/// EventSink::record + flush follows the same store.write() path.
/// Verifies that without redaction the secret survives the buffered flush.
#[test]
fn event_sink_flush_without_redaction_secret_visible_on_disk() {
    use braid_ports::EventSink;

    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let event = secret_tool_event("audit-sink-raw");

    store.record(&event).unwrap();
    store.flush().unwrap();

    let id = SessionId("audit-sink-raw".into());
    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        on_disk.contains(SECRET),
        "buffered flush without redaction leaves secret on disk"
    );
}

/// Redacting before record() ensures the buffered flush path is also clean.
#[test]
fn event_sink_flush_with_redaction_secret_absent_from_disk() {
    use braid_ports::EventSink;

    let dir = tempdir().unwrap();
    let store = SessionStore::open(dir.path().to_path_buf()).unwrap();
    let pipeline = RedactionPipeline::new().with_rule(SecretPatternRule::new());

    let redacted = pipeline.redact_event(&secret_tool_event("audit-sink-redacted"));
    store.record(&redacted).unwrap();
    store.flush().unwrap();

    let id = SessionId("audit-sink-redacted".into());
    let on_disk = read_events_jsonl(&store, &id);
    assert!(
        !on_disk.contains(SECRET),
        "redact_event before record() must prevent secret reaching disk via flush"
    );
    assert!(
        on_disk.contains(REDACTED_TOKEN),
        "redaction token must be present after buffered flush"
    );
}
