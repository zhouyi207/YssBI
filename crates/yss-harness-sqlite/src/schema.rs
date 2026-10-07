//! Exact schema owned by this adapter.

pub(crate) const SCHEMA: &[&str] = &[
    r#"CREATE TABLE assistant_session (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
    )"#,
    r#"CREATE TABLE assistant_turn (
        id TEXT PRIMARY KEY NOT NULL,
        session_id TEXT NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE assistant_event (
        session_id TEXT NOT NULL,
        sequence INTEGER NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        PRIMARY KEY(session_id, sequence),
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE workflow_definition (
        id TEXT NOT NULL,
        version TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        PRIMARY KEY(id, version)
    )"#,
    r#"CREATE TABLE workflow_run (
        id TEXT PRIMARY KEY NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
    )"#,
    r#"CREATE TABLE tool_invocation (
        id TEXT PRIMARY KEY NOT NULL,
        idempotency_key TEXT NOT NULL UNIQUE,
        session_id TEXT NOT NULL,
        state TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        FOREIGN KEY(session_id) REFERENCES assistant_session(id)
    )"#,
    r#"CREATE TABLE knowledge_source (
        id TEXT PRIMARY KEY NOT NULL,
        status TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
    )"#,
    r#"CREATE TABLE knowledge_document (
        id TEXT PRIMARY KEY NOT NULL,
        source_id TEXT NOT NULL,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
        FOREIGN KEY(source_id) REFERENCES knowledge_source(id)
    )"#,
    r#"CREATE TABLE approval_grant (
        id TEXT PRIMARY KEY NOT NULL,
        consumed_at INTEGER,
        payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
    )"#,
];
