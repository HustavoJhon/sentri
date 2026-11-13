-- Latest normalized data detected from sources (JSON of the domain model).
-- `opportunities` stores the effective values (detected + manual overrides)
-- so that filters and search honor the user's corrections.
CREATE TABLE opportunity_detected (
    opportunity_id  INTEGER PRIMARY KEY REFERENCES opportunities (id) ON DELETE CASCADE,
    data            TEXT    NOT NULL,
    updated_at      INTEGER NOT NULL
);
