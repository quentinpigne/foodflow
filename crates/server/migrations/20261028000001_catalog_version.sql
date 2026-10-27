-- Version du référentiel (familles, aliments, produits) : le serveur garde le référentiel en mémoire et ne le
-- relit que si cette version a changé. Tenue par des déclencheurs, pour qu'aucune écriture ne l'oublie ; une
-- valeur aléatoire, pour qu'une transaction annulée ne puisse pas réutiliser une version déjà mise en mémoire.

CREATE TABLE catalog_version (
    id      INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL
) STRICT;

INSERT INTO catalog_version (id, version) VALUES (1, random());

CREATE TRIGGER food_group_insert_version AFTER INSERT ON food_group
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER food_group_update_version AFTER UPDATE ON food_group
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER food_group_delete_version AFTER DELETE ON food_group
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;

CREATE TRIGGER food_insert_version AFTER INSERT ON food
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER food_update_version AFTER UPDATE ON food
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER food_delete_version AFTER DELETE ON food
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;

CREATE TRIGGER product_insert_version AFTER INSERT ON product
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER product_update_version AFTER UPDATE ON product
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
CREATE TRIGGER product_delete_version AFTER DELETE ON product
BEGIN UPDATE catalog_version SET version = random() WHERE id = 1; END;
