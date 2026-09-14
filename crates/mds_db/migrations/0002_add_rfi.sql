-- Adds the `rfi` table (RFI-01/02) for databases that already reached
-- schema version 1 before this table existed in 0001_initial.sql.
--
-- Why this is a separate migration instead of just editing 0001 further:
-- rusqlite_migration tracks progress via `PRAGMA user_version`, not by
-- diffing SQL text. A database that already ran migration 1 has
-- `user_version = 1` and will never re-run 0001_initial.sql, no matter
-- what gets added to that file afterward — confirmed by finding exactly
-- this happen to a real local database created before the `rfi` table was
-- (mistakenly) added directly to 0001 instead of as a new migration: it
-- was stuck at `user_version = 1` with no `rfi` table at all. `IF NOT
-- EXISTS` on both statements makes this migration a safe no-op for a
-- brand-new database, where 0001_initial.sql already created `rfi`
-- directly (that redundancy is left in place there rather than removed,
-- since removing it would just recreate the same "silent no-op on already
-- -migrated databases" trap this migration exists to fix).
CREATE TABLE IF NOT EXISTS rfi (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES document(id) ON DELETE CASCADE,
    page_id TEXT REFERENCES page(id) ON DELETE SET NULL,
    markup_id TEXT REFERENCES markup(id) ON DELETE SET NULL,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    response TEXT,
    created_by TEXT REFERENCES user(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (document_id, number)
);

CREATE INDEX IF NOT EXISTS idx_rfi_document_id ON rfi(document_id);
CREATE INDEX IF NOT EXISTS idx_rfi_page_id ON rfi(page_id);
