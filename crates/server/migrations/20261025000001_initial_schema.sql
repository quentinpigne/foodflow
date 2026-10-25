-- Schéma initial (phase 1) : socle, aliments (spec 01) et stock (spec 02), en tables STRICT.
-- Conventions (docs/modele-de-donnees.md) : Id = TEXT de 36 caractères, Amount et Money = INTEGER,
-- Date = TEXT 'YYYY-MM-DD', Timestamp = TEXT ISO 8601 UTC, booléen = INTEGER 0/1, enum = TEXT + CHECK.

-- ===================================================================== Utilisateur, sessions, réglages

CREATE TABLE app_user (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    display_name  TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 80),
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE session (
    id           TEXT PRIMARY KEY CHECK (length(id) = 36),
    user_id      TEXT NOT NULL REFERENCES app_user (id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    device_label TEXT,
    expires_at   TEXT NOT NULL CHECK (expires_at LIKE '%Z'),
    last_seen_at TEXT NOT NULL CHECK (last_seen_at LIKE '%Z'),
    created_at   TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at   TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE app_setting (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL CHECK (json_valid(value)),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

-- ===================================================================== Aliments (spec 01)

CREATE TABLE nutrient (
    code          TEXT PRIMARY KEY CHECK (code = upper(code)),
    name          TEXT NOT NULL,
    storage_unit  TEXT NOT NULL CHECK (storage_unit IN ('CAL', 'MG', 'UG')),
    ciqual_code   TEXT UNIQUE,
    off_key       TEXT,
    position      INTEGER NOT NULL
) STRICT;

CREATE TABLE food_group (
    id                      TEXT PRIMARY KEY CHECK (length(id) = 36),
    parent_id               TEXT REFERENCES food_group (id),
    name                    TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    name_key                TEXT NOT NULL,
    ciqual_code             TEXT UNIQUE,
    nutrition_group         TEXT CHECK (nutrition_group IN (
        'VEGETABLES', 'FRUITS', 'LEGUMES', 'WHOLE_GRAINS', 'REFINED_STARCHES', 'DAIRY', 'POULTRY', 'RED_MEAT',
        'PROCESSED_MEAT', 'FISH', 'FATTY_FISH', 'EGGS', 'NUTS', 'ADDED_FATS', 'SWEET_PRODUCTS', 'SALTY_SNACKS',
        'SUGARY_DRINKS', 'OTHER')),
    stock_measure           TEXT CHECK (stock_measure IN ('MASS', 'VOLUME', 'COUNT')),
    tracking_mode           TEXT CHECK (tracking_mode IN ('QUANTITY', 'PRESENCE')),
    shelf_life_days         INTEGER CHECK (shelf_life_days >= 0),
    opened_shelf_life_days  INTEGER CHECK (opened_shelf_life_days >= 0),
    frozen_shelf_life_days  INTEGER CHECK (frozen_shelf_life_days >= 0),
    thawed_shelf_life_days  INTEGER CHECK (thawed_shelf_life_days >= 0),
    portion_measure         TEXT CHECK (portion_measure IN ('MASS', 'VOLUME', 'COUNT')),
    portion_amount          INTEGER CHECK (portion_amount > 0),
    reference_food_id       TEXT REFERENCES food (id),
    user_modified           INTEGER NOT NULL DEFAULT 0 CHECK (user_modified IN (0, 1)),
    created_at              TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at              TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((portion_measure IS NULL) = (portion_amount IS NULL))
) STRICT;

CREATE INDEX food_group_parent ON food_group (parent_id);

CREATE TABLE food (
    id                      TEXT PRIMARY KEY CHECK (length(id) = 36),
    group_id                TEXT NOT NULL REFERENCES food_group (id),
    name                    TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    name_key                TEXT NOT NULL UNIQUE,
    ciqual_code             TEXT UNIQUE,
    hidden                  INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
    nutrient_basis          TEXT NOT NULL DEFAULT 'PER_100_G' CHECK (nutrient_basis IN ('PER_100_G', 'PER_100_ML')),
    density_mg_per_ml       INTEGER CHECK (density_mg_per_ml > 0),
    piece_weight_mg         INTEGER CHECK (piece_weight_mg > 0),
    nutrition_group         TEXT CHECK (nutrition_group IN (
        'VEGETABLES', 'FRUITS', 'LEGUMES', 'WHOLE_GRAINS', 'REFINED_STARCHES', 'DAIRY', 'POULTRY', 'RED_MEAT',
        'PROCESSED_MEAT', 'FISH', 'FATTY_FISH', 'EGGS', 'NUTS', 'ADDED_FATS', 'SWEET_PRODUCTS', 'SALTY_SNACKS',
        'SUGARY_DRINKS', 'OTHER')),
    stock_measure           TEXT CHECK (stock_measure IN ('MASS', 'VOLUME', 'COUNT')),
    tracking_mode           TEXT CHECK (tracking_mode IN ('QUANTITY', 'PRESENCE')),
    shelf_life_days         INTEGER CHECK (shelf_life_days >= 0),
    opened_shelf_life_days  INTEGER CHECK (opened_shelf_life_days >= 0),
    frozen_shelf_life_days  INTEGER CHECK (frozen_shelf_life_days >= 0),
    thawed_shelf_life_days  INTEGER CHECK (thawed_shelf_life_days >= 0),
    portion_measure         TEXT CHECK (portion_measure IN ('MASS', 'VOLUME', 'COUNT')),
    portion_amount          INTEGER CHECK (portion_amount > 0),
    minimum_stock           INTEGER CHECK (minimum_stock >= 0),
    preferred_product_id    TEXT REFERENCES product (id) ON DELETE SET NULL,
    merged_into_id          TEXT REFERENCES food (id),
    user_modified           INTEGER NOT NULL DEFAULT 0 CHECK (user_modified IN (0, 1)),
    created_at              TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at              TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((portion_measure IS NULL) = (portion_amount IS NULL))
) STRICT;

CREATE INDEX food_group_id ON food (group_id);

CREATE TABLE food_alias (
    food_id    TEXT NOT NULL REFERENCES food (id) ON DELETE CASCADE,
    alias      TEXT NOT NULL CHECK (length(alias) BETWEEN 1 AND 200),
    alias_key  TEXT NOT NULL UNIQUE,
    PRIMARY KEY (food_id, alias_key)
) STRICT;

CREATE TABLE food_nutrient (
    food_id          TEXT NOT NULL REFERENCES food (id) ON DELETE CASCADE,
    nutrient_code    TEXT NOT NULL REFERENCES nutrient (code),
    value            INTEGER NOT NULL CHECK (value >= 0),
    estimated        INTEGER NOT NULL DEFAULT 0 CHECK (estimated IN (0, 1)),
    source           TEXT NOT NULL CHECK (source IN ('CIQUAL', 'OFF', 'MANUAL', 'COMPUTED')),
    reference_value  INTEGER CHECK (reference_value >= 0),
    PRIMARY KEY (food_id, nutrient_code)
) STRICT;

CREATE TABLE unit (
    id            TEXT PRIMARY KEY CHECK (length(id) = 36),
    food_id       TEXT REFERENCES food (id) ON DELETE CASCADE,
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    plural        TEXT CHECK (plural IS NULL OR length(plural) BETWEEN 1 AND 60),
    abbreviation  TEXT CHECK (abbreviation IS NULL OR length(abbreviation) BETWEEN 1 AND 20),
    measure       TEXT NOT NULL CHECK (measure IN ('MASS', 'VOLUME', 'COUNT')),
    amount        INTEGER NOT NULL CHECK (amount > 0),
    position      INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE INDEX unit_food ON unit (food_id);

CREATE TABLE product (
    id               TEXT PRIMARY KEY CHECK (length(id) = 36),
    food_id          TEXT REFERENCES food (id),
    group_id         TEXT REFERENCES food_group (id),
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    brand            TEXT CHECK (brand IS NULL OR length(brand) BETWEEN 1 AND 120),
    barcode          TEXT UNIQUE CHECK (barcode IS NULL OR length(barcode) IN (8, 12, 13)),
    stock_measure    TEXT CHECK (stock_measure IN ('MASS', 'VOLUME', 'COUNT')),
    package_amount   INTEGER CHECK (package_amount > 0),
    package_units    INTEGER CHECK (package_units > 0),
    nutrient_basis   TEXT NOT NULL DEFAULT 'PER_100_G' CHECK (nutrient_basis IN ('PER_100_G', 'PER_100_ML')),
    off_payload      TEXT CHECK (off_payload IS NULL OR json_valid(off_payload)),
    off_fetched_at   TEXT CHECK (off_fetched_at IS NULL OR off_fetched_at LIKE '%Z'),
    created_at       TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at       TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((food_id IS NULL) <> (group_id IS NULL)),
    CHECK (group_id IS NULL OR stock_measure IS NOT NULL)
) STRICT;

CREATE INDEX product_food ON product (food_id);

CREATE TABLE product_nutrient (
    product_id     TEXT NOT NULL REFERENCES product (id) ON DELETE CASCADE,
    nutrient_code  TEXT NOT NULL REFERENCES nutrient (code),
    value          INTEGER NOT NULL CHECK (value >= 0),
    estimated      INTEGER NOT NULL DEFAULT 0 CHECK (estimated IN (0, 1)),
    source         TEXT NOT NULL CHECK (source IN ('CIQUAL', 'OFF', 'MANUAL', 'COMPUTED')),
    PRIMARY KEY (product_id, nutrient_code)
) STRICT;

-- ===================================================================== Stock (spec 02)

CREATE TABLE location (
    id          TEXT PRIMARY KEY CHECK (length(id) = 36),
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    name_key    TEXT NOT NULL UNIQUE,
    kind        TEXT NOT NULL CHECK (kind IN ('PANTRY', 'FRIDGE', 'FREEZER')),
    position    INTEGER NOT NULL DEFAULT 0,
    soon_days   INTEGER CHECK (soon_days >= 0),
    created_at  TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at  TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;

CREATE TABLE stock_lot (
    id             TEXT PRIMARY KEY CHECK (length(id) = 36),
    product_id     TEXT REFERENCES product (id),
    food_id        TEXT REFERENCES food (id),
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
    CHECK ((product_id IS NULL) <> (food_id IS NULL)),
    CHECK (expiry_kind = 'NONE' OR expiry_date IS NOT NULL OR expiry_missing = 1)
) STRICT;

CREATE INDEX stock_lot_location ON stock_lot (location_id, depleted);
CREATE INDEX stock_lot_food ON stock_lot (food_id);
CREATE INDEX stock_lot_product ON stock_lot (product_id);

CREATE TABLE stock_movement (
    id                TEXT PRIMARY KEY CHECK (length(id) = 36),
    lot_id            TEXT NOT NULL REFERENCES stock_lot (id),
    date              TEXT NOT NULL CHECK (date = date(date)),
    kind              TEXT NOT NULL CHECK (kind IN (
        'PURCHASE', 'COOKED', 'LEFTOVER', 'PREPARED', 'WASTE', 'MOVE', 'ADJUSTMENT', 'INVENTORY', 'OPENED', 'CHECKED')),
    amount            INTEGER,
    presence          TEXT CHECK (presence IN ('AVAILABLE', 'LOW', 'OUT')),
    shortfall         INTEGER,
    waste_reason      TEXT CHECK (waste_reason IN ('EXPIRED', 'SPOILED', 'OTHER')),
    from_location_id  TEXT REFERENCES location (id),
    to_location_id    TEXT REFERENCES location (id),
    checked_until     TEXT CHECK (checked_until IS NULL OR checked_until = date(checked_until)),
    reverses_id       TEXT UNIQUE REFERENCES stock_movement (id),
    batch_id          TEXT CHECK (batch_id IS NULL OR length(batch_id) = 36),
    note              TEXT,
    created_at        TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    CHECK (kind <> 'WASTE' OR reverses_id IS NOT NULL OR waste_reason IS NOT NULL),
    CHECK (kind <> 'CHECKED' OR reverses_id IS NOT NULL OR checked_until > date)
) STRICT;

CREATE INDEX stock_movement_lot ON stock_movement (lot_id, date);
CREATE INDEX stock_movement_batch ON stock_movement (batch_id);
