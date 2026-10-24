# Modèle de données

Statut : **Brouillon** (2026-10-25)

Modèle **logique**. Sa traduction physique (SQLite, sqlx, tables `STRICT`) suit ADR-0009, avec les
conventions de Cashflow. Ce document est la source de vérité du schéma. Les règles métier détaillées
sont dans les specs ; ici ne figurent que les contraintes structurelles.

Domaines couverts à ce stade : aliments (spec 01) et stock (spec 02). Les autres viendront avec leur spec.

## Types logiques

| Type | Représentation | Référence |
|---|---|---|
| `Id` | UUIDv7, généré côté application | ADR-0009 |
| `Amount` | Entier signé 64 bits dans l'unité de stockage d'une mesure : mg (`MASS`), µL (`VOLUME`), millième de pièce (`COUNT`), ou millième de portion (lot de préparation) | ADR-0001 |
| `NutrientValue` | Entier ≥ 0 dans l'unité de stockage du nutriment (cal, mg ou µg), pour 100 g ou 100 mL | ADR-0001 |
| `Money` | Entier signé 64 bits, en centimes d'euro | ADR-0001 |
| `Ratio` | Entier en millièmes (1,000 → `1000`) | ADR-0001 |
| `Date` | Date calendaire `YYYY-MM-DD`, sans heure ni fuseau | ADR-0009 |
| `Timestamp` | Instant UTC, réservé à l'audit | ADR-0009 |
| `Json` | Document JSON validé par un schéma propre au champ | — |

Correspondance physique : celle de Cashflow (`Id` en `TEXT` de 36 caractères, entiers en `INTEGER`,
enums en `TEXT` avec `CHECK`, booléens en `INTEGER` 0/1, `Date` avec `CHECK (col = date(col))`).

Les réglages hérités (R-ALI-2) sont des colonnes **nullables** sur `food_group` et `food` :
`NULL` signifie « hérité ». Les familles racines ont toutes leurs valeurs.

## Aliments — spec 01

### `FoodGroup`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `parent_id` | `Id` ? | FK `FoodGroup` ; sans cycle (R-ALI-1) |
| `name` | texte | unique parmi les enfants d'un même parent |
| `ciqual_code` | texte ? | unique ; famille créée par l'import Ciqual |
| `nutrition_group` | `NutritionGroup` ? | hérité si `NULL` (spec `NUT`) |
| `stock_measure` | `Measure` ? | hérité si `NULL` |
| `tracking_mode` | `TrackingMode` ? | hérité si `NULL` |
| `shelf_life_days` | entier ? | sans date (`NONE`), hérité si `NULL` |
| `opened_shelf_life_days` | entier ? | hérité si `NULL` |
| `frozen_shelf_life_days` | entier ? | hérité si `NULL` |
| `thawed_shelf_life_days` | entier ? | hérité si `NULL` |
| `user_modified` | booléen | nom ou parent modifié à la main (R-ALI-51) |

### `Food`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `group_id` | `Id` | FK `FoodGroup` |
| `name` | texte | unique sans accents ni casse (R-ALI-10) |
| `ciqual_code` | texte ? | unique |
| `hidden` | booléen | aliment Ciqual non affiché (R-ALI-52) |
| `nutrient_basis` | `PER_100_G` \| `PER_100_ML` | `PER_100_G` pour Ciqual |
| `density_mg_per_ml` | entier ? | > 0 |
| `piece_weight_mg` | entier ? | > 0 |
| `stock_measure`, `tracking_mode`, `*_shelf_life_days` | | comme `FoodGroup`, hérités si `NULL` |
| `minimum_stock` | `Amount` ? | dans la mesure de stock ; `0` signifie « toujours en avoir » en `PRESENCE` (R-STK-42) |
| `preferred_product_id` | `Id` ? | FK `Product` rattaché à cet aliment (R-ALI-43) |
| `merged_into_id` | `Id` ? | FK `Food` ; aliment absorbé par une fusion (R-ALI-13) |

### `FoodAlias`

| Champ | Type | Contraintes |
|---|---|---|
| `food_id` | `Id` | FK `Food` |
| `alias` | texte | unique sans accents ni casse, tous aliments confondus |

### `Nutrient`

Liste de référence, fournie par une migration.

| Champ | Type | Contraintes |
|---|---|---|
| `code` | texte | PK (`ENERGY`, `PROTEIN`, `CARBOHYDRATE`, `SUGARS`, `FAT`, `SATURATED_FAT`, `FIBER`, `SALT`, …) |
| `storage_unit` | `CAL` \| `MG` \| `UG` | |
| `ciqual_code` | texte ? | code du constituant dans Ciqual |
| `off_key` | texte ? | clé dans `nutriments` d'Open Food Facts |

### `FoodNutrient` et `ProductNutrient`

| Champ | Type | Contraintes |
|---|---|---|
| `food_id` / `product_id` | `Id` | FK ; PK avec `nutrient_code` |
| `nutrient_code` | texte | FK `Nutrient` |
| `value` | `NutrientValue` | une ligne absente = valeur inconnue (R-ALI-20) |
| `estimated` | booléen | R-ALI-21, R-ALI-24 |
| `source` | `CIQUAL` \| `OFF` \| `MANUAL` \| `COMPUTED` | |
| `reference_value` | `NutrientValue` ? | valeur Ciqual remplacée par une saisie (R-ALI-11) |

### `Unit` et `FoodUnit`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `food_id` | `Id` ? | FK `Food` ; `NULL` pour une unité générale (`Unit`) |
| `name`, `plural`, `abbreviation` | texte | unique par aliment |
| `measure` | `Measure` | |
| `amount` | `Amount` | valeur d'une unité dans sa mesure (c. à soupe : 15 000 µL) |

### `Product`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `food_id` | `Id` ? | FK `Food` |
| `group_id` | `Id` ? | FK `FoodGroup` ; exactement un de `food_id`, `group_id` (R-ALI-40) |
| `name`, `brand` | texte, texte ? | |
| `barcode` | texte ? | unique ; clé de contrôle valide (R-ALI-41) |
| `stock_measure` | `Measure` ? | requis si `group_id` ; sinon celle de l'aliment |
| `package_amount` | `Amount` ? | conditionnement (R-ALI-42) |
| `package_units` | entier ? | nombre d'unités d'un lot vendu groupé |
| `nutrient_basis` | `PER_100_G` \| `PER_100_ML` | |
| `off_payload` | `Json` ? | réponse brute d'Open Food Facts (R-ALI-61) |
| `off_fetched_at` | `Timestamp` ? | |

### `OffCategoryMapping`

| Champ | Type | Contraintes |
|---|---|---|
| `off_category` | texte | PK (`en:semi-skimmed-milks`) |
| `food_id` / `group_id` | `Id` ? | exactement un des deux (R-ALI-62) |

## Stock — spec 02

### `Location`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `name` | texte | unique |
| `kind` | `PANTRY` \| `FRIDGE` \| `FREEZER` | |
| `position` | entier | ordre d'affichage |

### `StockLot`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `product_id` / `food_id` / `recipe_id` | `Id` ? | exactement un des trois (R-STK-1) ; `recipe_id` pour un lot de préparation |
| `location_id` | `Id` | FK `Location` ; emplacement courant (le journal garde l'historique) |
| `tracking_mode` | `TrackingMode` | figé à la création (R-STK-20), modifié seulement par R-STK-21 |
| `expiry_kind` | `USE_BY` \| `BEST_BEFORE` \| `NONE` | |
| `expiry_date` | `Date` ? | requis sauf `NONE` |
| `opened_on`, `frozen_on`, `thawed_on` | `Date` ? | dérivés des mouvements, recopiés pour la lecture |
| `checked_until` | `Date` ? | nouvelle date limite du dernier `CHECKED` (R-STK-37), recopiée pour la lecture |
| `unit_price` | `Money` ? | prix payé pour le lot (spec `COU`) |
| `needs_review` | booléen | écart à vérifier (R-STK-3) |

La **quantité** et l'**état de présence** ne sont pas des colonnes : ils se calculent depuis le journal
(ADR-0002). `opened_on`, `frozen_on`, `thawed_on` et `checked_until` sont des copies de lecture, remises à jour par le
service à chaque mouvement concerné, et vérifiées par un test de cohérence avec le journal.

### `StockMovement`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `lot_id` | `Id` | FK `StockLot` |
| `date` | `Date` | ≤ aujourd'hui ; ≥ date du dernier `INVENTORY` du lot (R-STK-14) |
| `kind` | `MovementKind` | |
| `amount` | `Amount` ? | signé ; requis en `QUANTITY` ; quantité constatée pour `INVENTORY` ; `NULL` pour `OPENED` et `CHECKED` |
| `checked_until` | `Date` ? | requis si `CHECKED` ; > `date` |
| `presence` | `PresenceState` ? | requis en `PRESENCE` pour `PURCHASE`, `ADJUSTMENT`, `INVENTORY` |
| `shortfall` | `Amount` ? | quantité demandée non disponible (R-STK-3), ou écart d'inventaire (R-STK-40) |
| `waste_reason` | `EXPIRED` \| `SPOILED` \| `OTHER` ? | requis si `WASTE` |
| `from_location_id`, `to_location_id` | `Id` ? | requis si `MOVE` |
| `reverses_id` | `Id` ? | FK `StockMovement` ; unique (R-STK-11) |
| `batch_id` | `Id` ? | validation d'origine (session de courses, repas) ; annulée en bloc (R-STK-12) |
| `note` | texte ? | |
| `created_at` | `Timestamp` | |
