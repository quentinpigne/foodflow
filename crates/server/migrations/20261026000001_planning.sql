-- no-transaction
-- Phase 3 : planification (spec 05) et lots de préparation et de restes (STK-08).
--
-- Un lot peut désormais contenir une recette (lot de préparation ou de restes, R-STK-1) : la contrainte de
-- `stock_lot` change, ce que SQLite ne permet qu'en reconstruisant la table. La procédure est celle de la
-- documentation de SQLite (« Making Other Kinds Of Table Schema Changes ») : clés étrangères désactivées
-- le temps de la copie, vérifiées avant la validation. D'où une migration hors transaction automatique.

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE stock_lot_new (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    product_id     TEXT REFERENCES product (id),
    food_id        TEXT REFERENCES food (id),
    recipe_id      TEXT REFERENCES recipe (id),
    location_id    TEXT NOT NULL REFERENCES location (id),
    tracking_mode  TEXT NOT NULL CHECK (tracking_mode IN ('QUANTITY', 'PRESENCE')),
    measure        TEXT NOT NULL CHECK (measure IN ('MASS', 'VOLUME', 'COUNT')),
    expiry_kind    TEXT NOT NULL CHECK (expiry_kind IN ('USE_BY', 'BEST_BEFORE', 'NONE')),
    expiry_date    TEXT CHECK (expiry_date IS NULL OR expiry_date = date(expiry_date)),
    expiry_missing INTEGER NOT NULL DEFAULT 0 CHECK (expiry_missing IN (0, 1)),
    stocked_on     TEXT NOT NULL CHECK (stocked_on = date(stocked_on)),
    opened_on      TEXT CHECK (opened_on IS NULL OR opened_on = date(opened_on)),
    frozen_on      TEXT CHECK (frozen_on IS NULL OR frozen_on = date(frozen_on)),
    thawed_on      TEXT CHECK (thawed_on IS NULL OR thawed_on = date(thawed_on)),
    checked_until  TEXT CHECK (checked_until IS NULL OR checked_until = date(checked_until)),
    unit_price     INTEGER CHECK (unit_price >= 0),
    needs_review   INTEGER NOT NULL DEFAULT 0 CHECK (needs_review IN (0, 1)),
    depleted       INTEGER NOT NULL DEFAULT 0 CHECK (depleted IN (0, 1)),
    note           TEXT,
    created_at     TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at     TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((product_id IS NOT NULL) + (food_id IS NOT NULL) + (recipe_id IS NOT NULL) = 1),
    -- Un lot de préparation se compte en portions (mesure `COUNT`, millièmes de portion) ou dans la mesure
    -- du rendement de sa recette.
    CHECK (expiry_kind = 'NONE' OR expiry_date IS NOT NULL OR expiry_missing = 1)
) STRICT;

INSERT INTO stock_lot_new (
    id, product_id, food_id, recipe_id, location_id, tracking_mode, measure, expiry_kind, expiry_date,
    expiry_missing, stocked_on, opened_on, frozen_on, thawed_on, checked_until, unit_price, needs_review,
    depleted, note, created_at, updated_at
)
SELECT
    id, product_id, food_id, NULL, location_id, tracking_mode, measure, expiry_kind, expiry_date,
    expiry_missing, stocked_on, opened_on, frozen_on, thawed_on, checked_until, unit_price, needs_review,
    depleted, note, created_at, updated_at
FROM stock_lot;

DROP TABLE stock_lot;
ALTER TABLE stock_lot_new RENAME TO stock_lot;

CREATE INDEX stock_lot_location ON stock_lot (location_id, depleted);
CREATE INDEX stock_lot_food ON stock_lot (food_id);
CREATE INDEX stock_lot_product ON stock_lot (product_id);
CREATE INDEX stock_lot_recipe ON stock_lot (recipe_id);

-- ===================================================================== Foyer (ADR-0004)

CREATE TABLE member (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    name              TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 60),
    adult_equivalent  INTEGER NOT NULL CHECK (adult_equivalent > 0),
    active            INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    position          INTEGER NOT NULL,
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at        TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE preference (
    id         TEXT PRIMARY KEY CHECK (length(id) = 36),
    member_id  TEXT NOT NULL REFERENCES member (id) ON DELETE CASCADE,
    food_id    TEXT REFERENCES food (id) ON DELETE CASCADE,
    group_id   TEXT REFERENCES food_group (id) ON DELETE CASCADE,
    recipe_id  TEXT REFERENCES recipe (id) ON DELETE CASCADE,
    level      TEXT NOT NULL CHECK (level IN ('LOVES', 'LIKES', 'DISLIKES', 'REFUSES')),
    CHECK ((food_id IS NOT NULL) + (group_id IS NOT NULL) + (recipe_id IS NOT NULL) = 1)
) STRICT;

CREATE UNIQUE INDEX preference_target ON preference (member_id, coalesce(food_id, group_id, recipe_id));

-- ===================================================================== Semaine type (spec 05)

CREATE TABLE habitual_meal (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name        TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 100),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE habitual_meal_item (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    habitual_meal_id  TEXT NOT NULL REFERENCES habitual_meal (id) ON DELETE CASCADE,
    position          INTEGER NOT NULL,
    food_id           TEXT REFERENCES food (id),
    product_id        TEXT REFERENCES product (id),
    recipe_id         TEXT REFERENCES recipe (id),
    -- Par équivalent adulte (R-PLA-20) : dans la mesure de stock de l'aliment, en portions pour une recette.
    amount            INTEGER NOT NULL CHECK (amount > 0),
    UNIQUE (habitual_meal_id, position),
    CHECK ((food_id IS NOT NULL) + (product_id IS NOT NULL) + (recipe_id IS NOT NULL) = 1)
) STRICT;

CREATE TABLE slot_schedule (
    slot_id                   TEXT NOT NULL REFERENCES meal_slot (id) ON DELETE CASCADE,
    weekday                   INTEGER NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    available_active_minutes  INTEGER NOT NULL CHECK (available_active_minutes >= 0),
    earliest_start            TEXT NOT NULL CHECK (earliest_start GLOB '[0-2][0-9]:[0-5][0-9]'),
    ready_by                  TEXT NOT NULL CHECK (ready_by GLOB '[0-2][0-9]:[0-5][0-9]'),
    out_of_home               INTEGER NOT NULL DEFAULT 0 CHECK (out_of_home IN (0, 1)),
    habitual_meal_id          TEXT REFERENCES habitual_meal (id) ON DELETE SET NULL,
    PRIMARY KEY (slot_id, weekday),
    CHECK (earliest_start < ready_by)
) STRICT;

CREATE TABLE member_presence (
    member_id  TEXT NOT NULL REFERENCES member (id) ON DELETE CASCADE,
    slot_id    TEXT NOT NULL REFERENCES meal_slot (id) ON DELETE CASCADE,
    weekday    INTEGER NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    present    INTEGER NOT NULL CHECK (present IN (0, 1)),
    PRIMARY KEY (member_id, slot_id, weekday)
) STRICT;

CREATE TABLE slot_role (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    slot_id           TEXT NOT NULL REFERENCES meal_slot (id) ON DELETE CASCADE,
    position          INTEGER NOT NULL,
    name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    required          INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
    recipe_kinds      TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(recipe_kinds)),
    nutrition_groups  TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(nutrition_groups)),
    UNIQUE (slot_id, position)
) STRICT;

-- ===================================================================== Repas (spec 05)

CREATE TABLE meal (
    id                        TEXT PRIMARY KEY CHECK (length(id) = 36),
    date                      TEXT NOT NULL CHECK (date = date(date)),
    slot_id                   TEXT NOT NULL REFERENCES meal_slot (id),
    status                    TEXT NOT NULL CHECK (status IN ('PLANNED', 'DONE', 'SKIPPED', 'OUT_OF_HOME')),
    available_active_minutes  INTEGER NOT NULL CHECK (available_active_minutes >= 0),
    earliest_start            TEXT NOT NULL CHECK (earliest_start GLOB '[0-2][0-9]:[0-5][0-9]'),
    ready_by                  TEXT NOT NULL CHECK (ready_by GLOB '[0-2][0-9]:[0-5][0-9]'),
    locked                    INTEGER NOT NULL DEFAULT 0 CHECK (locked IN (0, 1)),
    validation_batch_id       TEXT CHECK (validation_batch_id IS NULL OR length(validation_batch_id) = 36),
    note                      TEXT,
    created_at                TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at                TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    UNIQUE (date, slot_id),
    CHECK (earliest_start < ready_by),
    CHECK ((status = 'DONE') = (validation_batch_id IS NOT NULL))
) STRICT;

CREATE INDEX meal_date ON meal (date);

CREATE TABLE meal_diner (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    meal_id           TEXT NOT NULL REFERENCES meal (id) ON DELETE CASCADE,
    member_id         TEXT REFERENCES member (id),
    guest_label       TEXT,
    adult_equivalent  INTEGER NOT NULL CHECK (adult_equivalent > 0),
    UNIQUE (meal_id, member_id)
) STRICT;

CREATE TABLE meal_item (
    id                    TEXT PRIMARY KEY CHECK (length(id) = 36),
    meal_id               TEXT NOT NULL REFERENCES meal (id) ON DELETE CASCADE,
    role_id               TEXT REFERENCES slot_role (id) ON DELETE SET NULL,
    position              INTEGER NOT NULL,
    source                TEXT NOT NULL CHECK (source IN ('COOK', 'LOT', 'FOOD')),
    recipe_id             TEXT REFERENCES recipe (id),
    -- COOK : portions cuisinées (R-PLA-13), en millièmes de portion.
    cooked_portions       INTEGER CHECK (cooked_portions > 0),
    -- LOT : le lot mangé, ou le reste prévu d'un élément cuisiné d'un repas antérieur (son lot n'existe
    -- qu'une fois ce repas validé).
    lot_id                TEXT REFERENCES stock_lot (id),
    -- Sans le plat d'origine, le reste prévu n'existe plus : l'élément qui le mangeait disparaît avec lui.
    leftover_of_item_id   TEXT REFERENCES meal_item (id) ON DELETE CASCADE,
    food_id               TEXT REFERENCES food (id),
    product_id            TEXT REFERENCES product (id),
    -- LOT : portions (ou quantité) servies ; FOOD : quantité.
    amount                INTEGER CHECK (amount > 0),
    per_adult_equivalent  INTEGER NOT NULL DEFAULT 0 CHECK (per_adult_equivalent IN (0, 1)),
    -- Validation (R-PLA-31) : part mangée, et lot des restes déclarés d'un élément cuisiné.
    eaten_ratio           INTEGER CHECK (eaten_ratio BETWEEN 0 AND 1000),
    leftover_lot_id       TEXT REFERENCES stock_lot (id),
    UNIQUE (meal_id, position),
    CHECK (source <> 'COOK' OR (recipe_id IS NOT NULL AND cooked_portions IS NOT NULL)),
    CHECK (source <> 'LOT' OR ((lot_id IS NOT NULL OR leftover_of_item_id IS NOT NULL) AND amount IS NOT NULL)),
    CHECK (source <> 'FOOD' OR ((food_id IS NOT NULL) + (product_id IS NOT NULL) = 1 AND amount IS NOT NULL))
) STRICT;

CREATE INDEX meal_item_leftover ON meal_item (leftover_of_item_id);

-- Consommation réelle d'un élément, enregistrée à la validation (R-PLA-31) : base du bilan réel.
CREATE TABLE meal_ingredient_use (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    meal_item_id  TEXT NOT NULL REFERENCES meal_item (id) ON DELETE CASCADE,
    food_id       TEXT REFERENCES food (id),
    product_id    TEXT REFERENCES product (id),
    recipe_id     TEXT REFERENCES recipe (id),
    amount        INTEGER NOT NULL CHECK (amount >= 0),
    measure       TEXT NOT NULL CHECK (measure IN ('MASS', 'VOLUME', 'COUNT')),
    -- `NULL` : utilisé sans sortie de stock (aliment non suivi ou absent, R-PLA-32).
    movement_id   TEXT REFERENCES stock_movement (id),
    CHECK ((food_id IS NOT NULL) + (product_id IS NOT NULL) + (recipe_id IS NOT NULL) = 1)
) STRICT;

PRAGMA foreign_key_check;

COMMIT;

PRAGMA foreign_keys = ON;
