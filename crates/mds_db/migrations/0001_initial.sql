-- Initial schema for the MDS Rebar PDF platform.
-- UUID (text) primary keys, FK enforcement, timestamps per docs/database/README.md.

CREATE TABLE user (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE project (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by TEXT NOT NULL REFERENCES user(id)
);

CREATE TABLE project_member (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES user(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (project_id, user_id)
);

CREATE TABLE document (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES project(id) ON DELETE SET NULL,
    file_path TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE document_version (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES document(id) ON DELETE RESTRICT,
    version_number INTEGER NOT NULL,
    file_snapshot_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by TEXT REFERENCES user(id),
    UNIQUE (document_id, version_number)
);

CREATE TABLE page (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES document(id) ON DELETE CASCADE,
    page_number INTEGER NOT NULL,
    width REAL NOT NULL,
    height REAL NOT NULL,
    rotation INTEGER NOT NULL DEFAULT 0,
    UNIQUE (document_id, page_number)
);

CREATE TABLE markup (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES page(id) ON DELETE CASCADE,
    type TEXT NOT NULL,
    geometry_json TEXT NOT NULL,
    style_json TEXT NOT NULL,
    author TEXT REFERENCES user(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    locked INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE markup_comment (
    id TEXT PRIMARY KEY,
    markup_id TEXT NOT NULL REFERENCES markup(id) ON DELETE CASCADE,
    author TEXT REFERENCES user(id),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE scale (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES page(id) ON DELETE CASCADE,
    ratio REAL NOT NULL,
    unit_system TEXT NOT NULL,
    calibrated_by TEXT REFERENCES user(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE measurement (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES page(id) ON DELETE CASCADE,
    scale_id TEXT REFERENCES scale(id),
    type TEXT NOT NULL,
    geometry_json TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT NOT NULL,
    label TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE takeoff_item (
    id TEXT PRIMARY KEY,
    measurement_id TEXT REFERENCES measurement(id) ON DELETE SET NULL,
    description TEXT NOT NULL,
    quantity REAL NOT NULL,
    unit TEXT NOT NULL,
    cost_per_unit REAL,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE recovery_state (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES document(id) ON DELETE CASCADE,
    snapshot_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_document_project_id ON document(project_id);
CREATE INDEX idx_document_version_document_id ON document_version(document_id);
CREATE INDEX idx_page_document_id ON page(document_id);
CREATE INDEX idx_markup_page_id ON markup(page_id);
CREATE INDEX idx_markup_comment_markup_id ON markup_comment(markup_id);
CREATE INDEX idx_scale_page_id ON scale(page_id);
CREATE INDEX idx_measurement_page_id ON measurement(page_id);
CREATE INDEX idx_takeoff_item_measurement_id ON takeoff_item(measurement_id);
CREATE INDEX idx_recovery_state_document_id ON recovery_state(document_id);
CREATE INDEX idx_project_member_user_id ON project_member(user_id);
