-- Normalized headline used to merge the same story arriving from several
-- feeds with different URLs (e.g. Google News redirect links).
ALTER TABLE opportunities ADD COLUMN news_key TEXT;
CREATE INDEX idx_opportunities_news_key ON opportunities (news_key) WHERE news_key IS NOT NULL;

-- Per-source health and HTTP cache validators, for incremental refresh and
-- back-off after repeated failures.
CREATE TABLE source_state (
    source_name           TEXT PRIMARY KEY,
    etag                  TEXT,
    last_modified         TEXT,
    last_success_at       INTEGER,
    last_error            TEXT,
    last_error_at         INTEGER,
    consecutive_failures  INTEGER NOT NULL DEFAULT 0,
    next_attempt_at       INTEGER
);

-- Audit log of stance reclassifications so they can be undone.
CREATE TABLE stance_reclassifications (
    id              INTEGER PRIMARY KEY,
    batch           TEXT    NOT NULL,
    opportunity_id  INTEGER NOT NULL REFERENCES opportunities (id) ON DELETE CASCADE,
    old_stance      TEXT,
    new_stance      TEXT    NOT NULL,
    at              INTEGER NOT NULL
);
CREATE INDEX idx_stance_reclassifications_batch ON stance_reclassifications (batch);
