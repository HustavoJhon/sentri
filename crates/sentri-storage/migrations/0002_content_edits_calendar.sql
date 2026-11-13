-- Publisher, content kind and news stance.
ALTER TABLE opportunities ADD COLUMN author_name   TEXT;
ALTER TABLE opportunities ADD COLUMN author_handle TEXT;
ALTER TABLE opportunities ADD COLUMN author_url    TEXT;
ALTER TABLE opportunities ADD COLUMN author_avatar TEXT;
ALTER TABLE opportunities ADD COLUMN content_kind  TEXT NOT NULL DEFAULT 'opportunity';
ALTER TABLE opportunities ADD COLUMN stance        TEXT;

CREATE INDEX idx_opportunities_content ON opportunities (content_kind, discovered_at DESC);

-- Manual edits, kept apart from detected data. The `opportunities` row holds
-- what SENTRI detected and normalized; each row here replaces one field when
-- reading. Collection never touches this table, and deleting a row restores
-- the detected value.
CREATE TABLE opportunity_overrides (
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    field           TEXT    NOT NULL,
    value           TEXT,
    edited_at       INTEGER NOT NULL,
    PRIMARY KEY (opportunity_id, field)
) WITHOUT ROWID;

-- Personal data that has no detected counterpart.
CREATE TABLE opportunity_notes (
    opportunity_id  INTEGER PRIMARY KEY REFERENCES opportunities (id) ON DELETE CASCADE,
    notes           TEXT,
    priority        TEXT,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE opportunity_custom_fields (
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    key             TEXT    NOT NULL,
    value           TEXT    NOT NULL,
    PRIMARY KEY (opportunity_id, key)
) WITHOUT ROWID;

-- SENTRI's own calendar. `google_event_id` is filled by the future Google
-- Calendar sync; entries without it exist only locally.
CREATE TABLE calendar_entries (
    id               INTEGER PRIMARY KEY,
    opportunity_id   INTEGER REFERENCES opportunities (id) ON DELETE SET NULL,
    kind             TEXT    NOT NULL,
    title            TEXT    NOT NULL,
    starts_at        INTEGER NOT NULL,
    ends_at          INTEGER,
    all_day          INTEGER NOT NULL DEFAULT 0,
    notes            TEXT,
    google_event_id  TEXT UNIQUE,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);

CREATE INDEX idx_calendar_entries_start ON calendar_entries (starts_at);
-- One entry per opportunity and date kind (event, deadline…): no duplicates.
CREATE UNIQUE INDEX idx_calendar_entries_opportunity
    ON calendar_entries (opportunity_id, kind) WHERE opportunity_id IS NOT NULL;
