-- Rattachement mémorisé d'une catégorie d'Open Food Facts à un aliment ou une famille (R-ALI-62).

CREATE TABLE off_category_mapping (
    off_category  TEXT PRIMARY KEY CHECK (length(off_category) BETWEEN 1 AND 200),
    food_id       TEXT REFERENCES food (id) ON DELETE CASCADE,
    group_id      TEXT REFERENCES food_group (id) ON DELETE CASCADE,
    updated_at    TEXT NOT NULL CHECK (updated_at LIKE '%Z'),
    CHECK ((food_id IS NULL) <> (group_id IS NULL))
) STRICT;
