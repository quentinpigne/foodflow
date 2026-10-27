-- Propositions de menus (spec 05, R-PLA-64) : générées par l'IA (ADR-0014), puis par la génération
-- déterministe (ADR-0013). Rien n'est écrit dans les repas avant l'acceptation.

CREATE TABLE generation_run (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    kind              TEXT NOT NULL CHECK (kind IN ('DETERMINISTIC', 'AI')),
    period_start      TEXT NOT NULL CHECK (period_start = date(period_start)),
    period_end        TEXT NOT NULL CHECK (period_end = date(period_end)),
    seed              INTEGER,
    prompt            TEXT,
    -- AI : données envoyées, telles qu'affichées avant l'envoi, et réponse brute.
    request_payload   TEXT CHECK (request_payload IS NULL OR json_valid(request_payload)),
    response_payload  TEXT,
    proposal          TEXT NOT NULL CHECK (json_valid(proposal)),
    status            TEXT NOT NULL CHECK (status IN ('PENDING', 'ACCEPTED', 'PARTIALLY_ACCEPTED', 'DISCARDED')),
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at        TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK (period_start <= period_end),
    CHECK (kind <> 'DETERMINISTIC' OR seed IS NOT NULL)
) STRICT;

CREATE INDEX generation_run_created ON generation_run (created_at);
