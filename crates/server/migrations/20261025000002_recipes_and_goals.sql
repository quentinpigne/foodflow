-- Phase 2 : recettes (spec 03) et objectifs nutritionnels (spec 04).

-- ===================================================================== Recettes (spec 03)

CREATE TABLE recipe (
    id                 TEXT PRIMARY KEY CHECK (length(id) = 36),
    name               TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    kind               TEXT NOT NULL CHECK (kind IN ('MAIN', 'SIDE', 'STARTER', 'DESSERT', 'BREAKFAST', 'SNACK', 'BASE')),
    description        TEXT,
    notes              TEXT,
    yield_kind         TEXT NOT NULL CHECK (yield_kind IN ('PORTIONS', 'AMOUNT')),
    yield_portions     INTEGER CHECK (yield_portions > 0),
    yield_measure      TEXT CHECK (yield_measure IN ('MASS', 'VOLUME', 'COUNT')),
    yield_amount       INTEGER CHECK (yield_amount > 0),
    active_minutes     INTEGER NOT NULL DEFAULT 0 CHECK (active_minutes >= 0),
    passive_minutes    INTEGER NOT NULL DEFAULT 0 CHECK (passive_minutes >= 0),
    fridge_days        INTEGER CHECK (fridge_days > 0),
    freezable          INTEGER NOT NULL DEFAULT 0 CHECK (freezable IN (0, 1)),
    frozen_days        INTEGER CHECK (frozen_days > 0),
    finishing_minutes  INTEGER CHECK (finishing_minutes >= 0),
    leftover_food_id   TEXT REFERENCES food (id),
    source_url         TEXT,
    image_path         TEXT,
    archived           INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    created_at         TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at         TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((yield_kind = 'PORTIONS') = (yield_portions IS NOT NULL)),
    CHECK ((yield_kind = 'AMOUNT') = (yield_measure IS NOT NULL AND yield_amount IS NOT NULL)),
    CHECK (freezable = 0 OR frozen_days IS NOT NULL)
) STRICT;

CREATE INDEX recipe_source_url ON recipe (source_url);

CREATE TABLE recipe_tag (
    recipe_id  TEXT NOT NULL REFERENCES recipe (id) ON DELETE CASCADE,
    tag        TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 60),
    PRIMARY KEY (recipe_id, tag)
) STRICT;

CREATE TABLE recipe_ingredient (
    id                  TEXT PRIMARY KEY CHECK (length(id) = 36),
    recipe_id           TEXT NOT NULL REFERENCES recipe (id) ON DELETE CASCADE,
    position            INTEGER NOT NULL,
    section             TEXT,
    food_id             TEXT REFERENCES food (id),
    group_id            TEXT REFERENCES food_group (id),
    sub_recipe_id       TEXT REFERENCES recipe (id),
    amount              INTEGER CHECK (amount > 0),
    measure             TEXT CHECK (measure IN ('MASS', 'VOLUME', 'COUNT')),
    to_taste            INTEGER NOT NULL DEFAULT 0 CHECK (to_taste IN (0, 1)),
    entered_quantity    INTEGER CHECK (entered_quantity > 0),
    entered_unit_id     TEXT REFERENCES unit (id) ON DELETE SET NULL,
    entered_unit_label  TEXT,
    note                TEXT,
    optional            INTEGER NOT NULL DEFAULT 0 CHECK (optional IN (0, 1)),
    fixed               INTEGER NOT NULL DEFAULT 0 CHECK (fixed IN (0, 1)),
    source_line         TEXT,
    UNIQUE (recipe_id, position),
    CHECK ((food_id IS NOT NULL) + (group_id IS NOT NULL) + (sub_recipe_id IS NOT NULL) = 1),
    CHECK (to_taste = 0 OR (amount IS NULL AND entered_quantity IS NULL)),
    CHECK (sub_recipe_id IS NULL OR sub_recipe_id <> recipe_id)
) STRICT;

CREATE INDEX recipe_ingredient_food ON recipe_ingredient (food_id);
CREATE INDEX recipe_ingredient_sub ON recipe_ingredient (sub_recipe_id);

CREATE TABLE recipe_step (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    recipe_id      TEXT NOT NULL REFERENCES recipe (id) ON DELETE CASCADE,
    position       INTEGER NOT NULL,
    section        TEXT,
    text           TEXT NOT NULL CHECK (length(text) >= 1),
    timer_minutes  INTEGER CHECK (timer_minutes > 0),
    UNIQUE (recipe_id, position)
) STRICT;

CREATE TABLE recipe_draft (
    id           TEXT PRIMARY KEY CHECK (length(id) = 36),
    source_url   TEXT,
    fetched_at   TEXT NOT NULL CHECK (fetched_at LIKE '%Z'),
    payload      TEXT NOT NULL CHECK (json_valid(payload)),
    raw_json_ld  TEXT NOT NULL CHECK (json_valid(raw_json_ld)),
    image_path   TEXT,
    created_at   TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at   TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE ingredient_mapping (
    normalized_name  TEXT PRIMARY KEY,
    food_id          TEXT REFERENCES food (id) ON DELETE CASCADE,
    group_id         TEXT REFERENCES food_group (id) ON DELETE CASCADE,
    recipe_id        TEXT REFERENCES recipe (id) ON DELETE CASCADE,
    updated_at       TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((food_id IS NOT NULL) + (group_id IS NOT NULL) + (recipe_id IS NOT NULL) = 1)
) STRICT;

-- ===================================================================== Objectifs nutritionnels (spec 04)

CREATE TABLE meal_slot (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    name          TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 60),
    position      INTEGER NOT NULL,
    energy_share  INTEGER NOT NULL CHECK (energy_share BETWEEN 0 AND 1000),
    active        INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE nutrition_goal (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    valid_from  TEXT NOT NULL UNIQUE CHECK (valid_from = date(valid_from)),
    note        TEXT,
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE goal_template (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 120),
    builtin     INTEGER NOT NULL DEFAULT 0 CHECK (builtin IN (0, 1)),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE goal_criterion (
    id                    TEXT PRIMARY KEY CHECK (length(id) = 36),
    goal_id               TEXT REFERENCES nutrition_goal (id) ON DELETE CASCADE,
    template_id           TEXT REFERENCES goal_template (id) ON DELETE CASCADE,
    kind                  TEXT NOT NULL CHECK (kind IN (
        'ENERGY', 'NUTRIENT', 'GROUP_PORTIONS', 'GROUP_AMOUNT', 'GROUP_FREQUENCY', 'RECIPE_REPEAT', 'FAMILY_REPEAT')),
    nutrient_code         TEXT REFERENCES nutrient (code),
    nutrition_groups      TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(nutrition_groups)),
    food_group_id         TEXT REFERENCES food_group (id) ON DELETE CASCADE,
    expression            TEXT NOT NULL CHECK (expression IN ('ABSOLUTE', 'PERCENT_ENERGY')),
    period                TEXT NOT NULL CHECK (period IN ('DAY', 'WEEK')),
    min_value             INTEGER CHECK (min_value >= 0),
    max_value             INTEGER CHECK (max_value >= 0),
    tolerance             INTEGER CHECK (tolerance >= 0),
    occurrence_threshold  INTEGER CHECK (occurrence_threshold > 0),
    importance            TEXT NOT NULL CHECK (importance IN ('REQUIRED', 'PREFERRED', 'INFO')),
    position              INTEGER NOT NULL,
    CHECK ((goal_id IS NOT NULL) <> (template_id IS NOT NULL)),
    CHECK (min_value IS NOT NULL OR max_value IS NOT NULL)
) STRICT;

CREATE INDEX goal_criterion_goal ON goal_criterion (goal_id);
CREATE INDEX goal_criterion_template ON goal_criterion (template_id);
