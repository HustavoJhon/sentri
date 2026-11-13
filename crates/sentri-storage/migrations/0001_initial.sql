-- SENTRI initial schema.
-- Timestamps are INTEGER unix seconds (UTC) so they sort and compare exactly.
-- Booleans that may be unknown are nullable INTEGER (NULL = not stated).

CREATE TABLE sources (
    id          INTEGER PRIMARY KEY,
    name        TEXT    NOT NULL UNIQUE,
    kind        TEXT    NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);

CREATE TABLE opportunities (
    id                     INTEGER PRIMARY KEY,
    fingerprint            TEXT    NOT NULL UNIQUE,
    canonical_url          TEXT    NOT NULL,
    original_url           TEXT    NOT NULL,
    source_kind            TEXT    NOT NULL,
    source_name            TEXT    NOT NULL,
    external_id            TEXT,
    title                  TEXT    NOT NULL,
    summary                TEXT,
    image_url              TEXT,
    category               TEXT    NOT NULL,
    location               TEXT,
    country                TEXT,
    modality               TEXT    NOT NULL,
    published_at           INTEGER,
    starts_at              INTEGER,
    ends_at                INTEGER,
    deadline               INTEGER,
    cost_kind              TEXT    NOT NULL,
    cost_detail            TEXT,
    requirements           TEXT,
    eligibility            TEXT    NOT NULL,
    certificate            INTEGER,
    prizes                 TEXT,
    benefits               TEXT,
    travel_covered         INTEGER,
    accommodation_covered  INTEGER,
    discovered_at          INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_opportunities_source_external
    ON opportunities (source_name, external_id) WHERE external_id IS NOT NULL;
CREATE INDEX idx_opportunities_category   ON opportunities (category);
CREATE INDEX idx_opportunities_discovered ON opportunities (discovered_at DESC);
CREATE INDEX idx_opportunities_deadline   ON opportunities (deadline) WHERE deadline IS NOT NULL;

CREATE TABLE opportunity_tags (
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    tag             TEXT    NOT NULL,
    PRIMARY KEY (opportunity_id, tag)
) WITHOUT ROWID;

CREATE INDEX idx_opportunity_tags_tag ON opportunity_tags (tag);

-- Every source an opportunity was seen in. Keeps external identifiers and
-- original URLs when the same item arrives from several places.
CREATE TABLE opportunity_sightings (
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    source_name     TEXT    NOT NULL,
    external_id     TEXT,
    original_url    TEXT    NOT NULL,
    first_seen_at   INTEGER NOT NULL,
    last_seen_at    INTEGER NOT NULL,
    PRIMARY KEY (opportunity_id, source_name)
) WITHOUT ROWID;

-- Current interaction state (one row per opportunity the user touched).
CREATE TABLE interactions (
    opportunity_id  INTEGER PRIMARY KEY REFERENCES opportunities (id) ON DELETE CASCADE,
    liked           INTEGER NOT NULL DEFAULT 0,
    saved           INTEGER NOT NULL DEFAULT 0,
    dismissed       INTEGER NOT NULL DEFAULT 0,
    interesting     INTEGER NOT NULL DEFAULT 0,
    applied         INTEGER NOT NULL DEFAULT 0,
    enrolled        INTEGER NOT NULL DEFAULT 0,
    completed       INTEGER NOT NULL DEFAULT 0,
    last_viewed_at  INTEGER,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_interactions_liked     ON interactions (opportunity_id) WHERE liked = 1;
CREATE INDEX idx_interactions_saved     ON interactions (opportunity_id) WHERE saved = 1;
CREATE INDEX idx_interactions_dismissed ON interactions (opportunity_id) WHERE dismissed = 1;

-- Append-only history of user actions.
CREATE TABLE interaction_events (
    id              INTEGER PRIMARY KEY,
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    action          TEXT    NOT NULL,
    at              INTEGER NOT NULL
);

CREATE INDEX idx_interaction_events_at ON interaction_events (at DESC);

-- One row per connector execution, for status and troubleshooting.
CREATE TABLE collection_runs (
    id           INTEGER PRIMARY KEY,
    source_name  TEXT    NOT NULL,
    started_at   INTEGER NOT NULL,
    finished_at  INTEGER,
    status       TEXT    NOT NULL,
    items_seen   INTEGER NOT NULL DEFAULT 0,
    items_new    INTEGER NOT NULL DEFAULT 0,
    error        TEXT
);

CREATE INDEX idx_collection_runs_source ON collection_runs (source_name, started_at DESC);
