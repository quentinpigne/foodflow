# Modèle de données

Statut : **Validée** (2026-10-25)

Modèle **logique**. Sa traduction physique (SQLite, sqlx, tables `STRICT`) suit ADR-0009, avec les
conventions de Cashflow. Ce document est la source de vérité du schéma. Les règles métier détaillées
sont dans les specs ; ici ne figurent que les contraintes structurelles.

Domaines couverts à ce stade : aliments (spec 01), stock (spec 02), recettes (spec 03) et objectifs
nutritionnels (spec 04), planification (spec 05), courses (spec 06),
anti-gaspillage (spec 07), lien avec Cashflow (spec 08). Les autres viendront avec leur spec.

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

Toutes les tables d'entités ont `created_at` et `updated_at` (`Timestamp`, audit), non répétés ci-dessous.

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
| `reference_food_id` | `Id` ? | FK `Food` de la famille ou d'une sous-famille (R-ALI-4) |
| `nutrition_group` | `NutritionGroup` ? | hérité si `NULL` (spec 04) |
| `aisle_id` | `Id` ? | FK `Aisle` ; rayon par défaut, hérité si `NULL` (R-COU-30) |
| `portion_measure`, `portion_amount` | `Measure` ?, `Amount` ? | taille de portion du groupe nutritionnel ; héritée si `NULL` (R-NUT-22) |
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
| `stock_measure`, `tracking_mode`, `*_shelf_life_days`, `nutrition_group`, `portion_*` | | comme `FoodGroup`, hérités si `NULL` |
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
| `soon_days` | entier ? | seuil `SOON` propre à l'emplacement (R-GAS-2) ; réglage général si `NULL` |
| `position` | entier | ordre d'affichage |

### `StockLot`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `product_id` / `food_id` / `recipe_id` | `Id` ? | exactement un des trois (R-STK-1) ; `recipe_id` pour un lot de préparation ou de restes, compté en portions (mesure `COUNT`, millièmes de portion) ou dans la mesure du rendement de sa recette |
| `location_id` | `Id` | FK `Location` ; emplacement courant (le journal garde l'historique) |
| `measure` | `Measure` | mesure de stock au moment de la création ; les quantités du lot s'y expriment |
| `stocked_on` | `Date` | entrée en stock ; point de départ d'une durée de conservation sans date (R-STK-30) |
| `expiry_missing` | booléen | date de péremption à compléter (R-COU-51) |
| `depleted` | booléen | lot épuisé (R-STK-4), copie de lecture tenue à jour à chaque mouvement |
| `tracking_mode` | `TrackingMode` | figé à la création (R-STK-20), modifié seulement par R-STK-21 |
| `expiry_kind` | `USE_BY` \| `BEST_BEFORE` \| `NONE` | |
| `expiry_date` | `Date` ? | requis sauf `NONE` |
| `opened_on`, `frozen_on`, `thawed_on` | `Date` ? | dérivés des mouvements, recopiés pour la lecture |
| `checked_until` | `Date` ? | nouvelle date limite du dernier `CHECKED` (R-STK-37), recopiée pour la lecture |
| `unit_price` | `Money` ? | prix payé pour le lot (spec 06) |
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

## Recettes — spec 03

### `Recipe`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `name` | texte | non vide |
| `kind` | `RecipeKind` | `MAIN`, `SIDE`, `STARTER`, `DESSERT`, `BREAKFAST`, `SNACK`, `BASE` |
| `description`, `notes` | texte ? | description d'origine, notes personnelles (R-REC-4) |
| `yield_kind` | `PORTIONS` \| `AMOUNT` | R-REC-10, R-REC-11 |
| `yield_portions` | `Amount` ? | millièmes de portion adulte, > 0 ; requis si `PORTIONS` |
| `yield_measure` | `Measure` ? | requis si `AMOUNT` |
| `yield_amount` | `Amount` ? | > 0 ; requis si `AMOUNT` |
| `active_minutes`, `passive_minutes` | entier | ≥ 0 (R-REC-40) |
| `fridge_days` | entier ? | > 0 ; présent = se prépare à l'avance (R-REC-42) |
| `freezable` | booléen | |
| `frozen_days` | entier ? | requis si `freezable` |
| `finishing_minutes` | entier ? | ≥ 0 |
| `leftover_food_id` | `Id` ? | FK `Food` ; aliment de reste (R-REC-44) |
| `source_url` | texte ? | URL d'origine d'un import |
| `image_path` | texte ? | fichier dans le volume `/data/images` |
| `archived` | booléen | R-REC-2 |
| `created_at`, `updated_at` | `Timestamp` | |

### `RecipePhase` (ADR-0015)

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `recipe_id` | `Id` | FK `Recipe` |
| `position` | entier | unique par recette |
| `kind` | `ACTIVE` \| `PASSIVE` | |
| `minutes` | entier | > 0 |
| `label` | texte ? | « Mijoter », « Enfourner » |
| `equipment_ids` | `Json` | liste d'identifiants `Equipment` |

Quand une recette a des phases, `active_minutes` et `passive_minutes` en sont la somme par nature.

### `Equipment`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `name` | texte | unique (« Four », « Feux », « Robot ») |
| `capacity` | entier | ≥ 1 |

### `RecipeTag`

| Champ | Type | Contraintes |
|---|---|---|
| `recipe_id` | `Id` | FK `Recipe` ; PK avec `tag` |
| `tag` | texte | normalisé (minuscules, sans espaces superflus) |

### `RecipeIngredient`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `recipe_id` | `Id` | FK `Recipe` |
| `position` | entier | ordre ; unique par recette |
| `section` | texte ? | « Pour la béchamel » |
| `food_id` / `group_id` / `sub_recipe_id` | `Id` ? | exactement un des trois (R-REC-20) ; sans cycle (R-REC-30) |
| `amount` | `Amount` ? | dans la mesure de stock, ou en portions ou unité de rendement de la sous-recette ; `NULL` si à volonté ou non convertible |
| `measure` | `Measure` ? | mesure de `amount` (masse, volume ou pièce) ; `NULL` pour des portions de sous-recette |
| `to_taste` | booléen | à volonté (R-REC-22) ; alors `amount` et la saisie sont vides |
| `entered_quantity` | `Ratio` ? | quantité saisie, en millièmes |
| `entered_unit_id` | `Id` ? | FK `Unit` (générale ou propre) |
| `entered_unit_label` | texte ? | unité saisie sans correspondance (« sachet ») : affichée telle quelle, `amount` vide |
| `note` | texte ? | précision (« émincé ») |
| `optional` | booléen | R-REC-23 |
| `fixed` | booléen | non mis à l'échelle (R-REC-13) |
| `source_line` | texte ? | ligne d'origine d'un import (R-REC-65) |

### `RecipeStep`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `recipe_id` | `Id` | FK `Recipe` |
| `position` | entier | unique par recette |
| `section` | texte ? | |
| `text` | texte | non vide |
| `timer_minutes` | entier ? | > 0 |

### `RecipeDraft`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `source_url` | texte ? | vide pour un fichier JSON-LD réimporté (REC-11) |
| `fetched_at` | `Timestamp` | |
| `payload` | `Json` | recette analysée et propositions de rattachement, modifiées pendant la revue |
| `raw_json_ld` | `Json` | données schema.org d'origine |
| `image_path` | texte ? | image téléchargée à l'import, reprise par la recette à sa validation |

Un brouillon est supprimé à sa validation (qui crée la recette) ou à son abandon (R-REC-61).

### `IngredientMapping`

| Champ | Type | Contraintes |
|---|---|---|
| `normalized_name` | texte | PK ; sans accents, minuscules, singulier (R-REC-66) |
| `food_id` / `group_id` / `recipe_id` | `Id` ? | exactement un des trois |
| `unit_id` | `Id` ? | FK `Unit` propre à l'aliment, si le nom en implique une (« gousses d'ail ») ; pas encore implémenté |
| `updated_at` | `Timestamp` | |

## Objectifs nutritionnels — spec 04

### `MealSlot`

Créneau de repas de la journée ; complété par la spec 05 (temps disponible, heure limite).

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `name` | texte | unique |
| `position` | entier | ordre dans la journée |
| `energy_share` | `Ratio` | part de l'énergie du jour ; Σ des créneaux actifs = 1 000 (R-NUT-30) |
| `active` | booléen | |

### `NutritionGoal`

Une ligne par version de l'objectif du foyer (R-NUT-1, R-NUT-2).

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `valid_from` | `Date` | unique |
| `note` | texte ? | |

### `GoalTemplate`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `name` | texte | unique |
| `builtin` | booléen | modèle fourni (« Repères PNNS 2019 — adulte »), non modifiable |

### `GoalCriterion`

Rattaché à une version (`goal_id`) **ou** à un modèle (`template_id`).

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `goal_id` / `template_id` | `Id` ? | exactement un des deux |
| `kind` | `CriterionKind` | `ENERGY`, `NUTRIENT`, `GROUP_PORTIONS`, `GROUP_AMOUNT`, `GROUP_FREQUENCY`, `RECIPE_REPEAT`, `FAMILY_REPEAT` |
| `nutrient_code` | texte ? | FK `Nutrient` ; requis si `ENERGY` (`ENERGY`) ou `NUTRIENT` |
| `nutrition_groups` | `Json` ? | liste de `NutritionGroup` ; requis si `GROUP_*` |
| `food_group_id` | `Id` ? | FK `FoodGroup` ; requis si `FAMILY_REPEAT` |
| `expression` | `ABSOLUTE` \| `PERCENT_ENERGY` | `PERCENT_ENERGY` seulement pour `NUTRIENT` |
| `period` | `DAY` \| `WEEK` | |
| `min_value`, `max_value` | entier ? | au moins un des deux ; unité du nutriment, millièmes de portion, `Amount`, ou jours (`RECIPE_REPEAT`) ; ‰ pour un pourcentage |
| `tolerance` | `Ratio` ? | requis si `ENERGY` (R-NUT-12) |
| `occurrence_threshold` | `Amount` ? | millièmes de portion ; `GROUP_FREQUENCY` (R-NUT-14) |
| `importance` | `REQUIRED` \| `PREFERRED` \| `INFO` | |
| `position` | entier | ordre d'affichage |

## Planification — spec 05

### `Member` et `Preference` (ADR-0004)

| Champ | Type | Contraintes |
|---|---|---|
| `Member.id` | `Id` | PK |
| `Member.name` | texte | unique |
| `Member.adult_equivalent` | `Ratio` | > 0 |
| `Member.active` | booléen | un membre parti du foyer reste dans l'historique |
| `Member.position` | entier | ordre d'affichage |
| `Preference.id` | `Id` | PK |
| `Preference.member_id` | `Id` | FK `Member` |
| `Preference.food_id` / `group_id` / `recipe_id` | `Id` ? | exactement un des trois ; unique par membre et cible |
| `Preference.level` | `LOVES` \| `LIKES` \| `DISLIKES` \| `REFUSES` | |

### `SlotSchedule` (semaine type)

| Champ | Type | Contraintes |
|---|---|---|
| `slot_id` | `Id` | FK `MealSlot` ; PK avec `weekday` |
| `weekday` | entier | 1 (lundi) à 7 |
| `available_active_minutes` | entier | ≥ 0 (R-PLA-1) |
| `earliest_start`, `ready_by` | heure `HH:MM` | `earliest_start` < `ready_by` |
| `out_of_home` | booléen | hors foyer par défaut (R-PLA-3) |
| `habitual_meal_id` | `Id` ? | FK `HabitualMeal` (R-PLA-21) |

### `MemberPresence`

| Champ | Type | Contraintes |
|---|---|---|
| `member_id`, `slot_id`, `weekday` | `Id`, `Id`, entier | PK ; absent = présent par défaut |
| `present` | booléen | R-PLA-2 |

### `SlotRole` (composition type)

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `slot_id` | `Id` | FK `MealSlot` |
| `position` | entier | unique par créneau |
| `name` | texte | « Plat », « Accompagnement », « Laitage ou fruit » |
| `required` | booléen | R-PLA-14 |
| `recipe_kinds` | `Json` ? | liste de `RecipeKind` admis |
| `nutrition_groups` | `Json` ? | liste de `NutritionGroup` admis pour un aliment |

### `HabitualMeal` et `HabitualMealItem`

| Champ | Type | Contraintes |
|---|---|---|
| `HabitualMeal.id`, `name` | `Id`, texte | nom unique |
| `HabitualMealItem.id`, `position` | `Id`, entier | position unique par repas habituel |
| `HabitualMealItem.habitual_meal_id` | `Id` | FK |
| `HabitualMealItem.food_id` / `product_id` / `recipe_id` | `Id` ? | exactement un des trois |
| `HabitualMealItem.amount` | `Amount` | **par équivalent adulte** (R-PLA-20) ; portions pour une recette |

### `Meal`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `date`, `slot_id` | `Date`, `Id` | unique ensemble (R-PLA-10) |
| `status` | `PLANNED` \| `DONE` \| `SKIPPED` \| `OUT_OF_HOME` | |
| `available_active_minutes`, `earliest_start`, `ready_by` | | recopiés de la semaine type, modifiables (R-PLA-5) |
| `locked` | booléen | R-PLA-15 |
| `validation_batch_id` | `Id` ? | mouvements créés par la validation (R-STK-12) |
| `note` | texte ? | |

### `MealDiner`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `meal_id` | `Id` | FK `Meal` |
| `member_id` | `Id` ? | FK `Member` ; `NULL` pour un invité ; unique par repas |
| `guest_label` | texte ? | |
| `adult_equivalent` | `Ratio` | recopié du membre, saisi pour un invité |

### `MealItem`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `meal_id` | `Id` | FK `Meal` |
| `role_id` | `Id` ? | FK `SlotRole` |
| `position` | entier | |
| `source` | `COOK` \| `PREPARED` \| `LOT` \| `FOOD` | R-PLA-12 ; `PREPARED` arrivera avec les sessions de préparation (PLA-10) |
| `recipe_id` | `Id` ? | requis si `COOK` ou `PREPARED` |
| `cooked_portions` | `Amount` ? | `COOK` : portions cuisinées (R-PLA-13) |
| `prep_item_id` | `Id` ? | FK `PrepItem` ; requis si `PREPARED` |
| `lot_id` | `Id` ? | FK `StockLot` ; `LOT` : le lot mangé, ou celui des restes de `leftover_of_item_id` une fois son repas validé |
| `food_id` / `product_id` | `Id` ? | un des deux requis si `FOOD` |
| `amount` | `Amount` ? | portions servies (`PREPARED`, `LOT`), ou quantité (`FOOD`) |
| `per_adult_equivalent` | booléen | `FOOD` : quantité par équivalent adulte ou totale |
| `leftover_of_item_id` | `Id` ? | FK `MealItem` (`COOK`, repas antérieur) dont c'est le reste prévu (R-PLA-13) ; supprimé avec lui |
| `eaten_ratio` | `Ratio` ? | renseigné à la validation : part mangée (R-PLA-31) |
| `leftover_lot_id` | `Id` ? | FK `StockLot` : lot des restes déclarés à la validation d'un élément `COOK` (ADR-0012) |

### `MealIngredientUse`

Consommation réelle, enregistrée à la validation (R-PLA-31), base du bilan réel.

| Champ | Type | Contraintes |
|---|---|---|
| `meal_item_id` | `Id` | FK `MealItem` |
| `food_id` / `product_id` / `recipe_id` | `Id` ? | exactement un des trois (`recipe_id` : part de lot de préparation) |
| `id` | `Id` | PK |
| `amount` | `Amount` | quantité utilisée |
| `measure` | `Measure` | mesure de `amount` (`COUNT` pour des portions) |
| `movement_id` | `Id` ? | FK `StockMovement` ; `NULL` si rien n'est sorti du stock (R-PLA-32) |

### `PrepSession` et `PrepItem` (ADR-0011)

| Champ | Type | Contraintes |
|---|---|---|
| `PrepSession.id` | `Id` | PK |
| `PrepSession.date`, `start_time`, `available_minutes` | `Date`, heure, entier | |
| `PrepSession.cooks` | entier | ≥ 1 (R-PLA-42) |
| `PrepSession.status` | `PLANNED` \| `DONE` \| `SKIPPED` | |
| `PrepSession.validation_batch_id` | `Id` ? | |
| `PrepItem.id`, `session_id`, `recipe_id` | `Id` | |
| `PrepItem.amount` | `Amount` | portions ou quantité de rendement (R-PLA-41) |
| `PrepItem.location_id` | `Id` | FK `Location` ; congélateur ⇒ recette congelable |
| `PrepItem.lot_id` | `Id` ? | lot créé à la validation (R-PLA-44) |

### `GenerationRun`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `kind` | `DETERMINISTIC` \| `AI` | |
| `period_start`, `period_end` | `Date` | |
| `seed` | entier ? | requis si `DETERMINISTIC` (R-PLA-65) |
| `settings` | `Json` | poids, itérations, plafond de coût, consigne libre |
| `request_payload` | `Json` ? | `AI` : données envoyées, telles qu'affichées (ADR-0014) |
| `response_payload` | `Json` ? | `AI` : réponse brute |
| `proposal` | `Json` | repas proposés, raisons, bilan, contraintes non satisfaites ; recettes nouvelles avec leur brouillon (`RecipeDraft`), puis la recette créée à sa validation |
| `prompt` | texte ? | consigne libre |
| `status` | `PENDING` \| `ACCEPTED` \| `PARTIALLY_ACCEPTED` \| `DISCARDED` | |
| `created_at` | `Timestamp` | |

## Courses — spec 06

### `Aisle`, `Store`, `StoreAisle`

| Champ | Type | Contraintes |
|---|---|---|
| `Aisle.id`, `name`, `position` | `Id`, texte, entier | nom unique ; `position` : ordre général |
| `Store.id`, `name` | `Id`, texte | nom unique |
| `StoreAisle.store_id`, `aisle_id`, `position` | `Id`, `Id`, entier | PK (`store_id`, `aisle_id`) ; ordre propre au magasin (R-COU-31) |

`Food` et `Product` ont aussi un `aisle_id` facultatif (R-COU-30).

### `ShoppingTrip`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `date` | `Date` | |
| `store_id` | `Id` ? | FK `Store` |
| `status` | `PLANNED` \| `IN_PROGRESS` \| `DONE` \| `CANCELLED` | au plus une session `IN_PROGRESS` |
| `covers_until` | `Date` | calculée au départ en magasin (R-COU-20) |
| `total_paid` | `Money` ? | R-COU-53 |
| `validation_batch_id` | `Id` ? | mouvements `PURCHASE` (R-COU-50) |
| `no_transaction` | booléen | payée sans transaction suivie dans Cashflow (R-CSH-36) |

### `ShoppingAdjustment`

Ajustements d'une session `PLANNED`, conservés au recalcul (R-COU-23).

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `trip_id` | `Id` | FK `ShoppingTrip` |
| `kind` | `ADD` \| `SET_AMOUNT` \| `REMOVE` | |
| `food_id` / `product_id` | `Id` ? | au plus un ; aucun pour un article libre (`ADD`) |
| `label` | texte ? | requis pour un article libre |
| `amount` | `Amount` ? | requis pour `SET_AMOUNT` |
| `aisle_id` | `Id` ? | |

### `ShoppingItem`

Articles figés au départ en magasin (R-COU-40).

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK, généré par l'appareil pour un ajout hors connexion |
| `trip_id` | `Id` | FK `ShoppingTrip` |
| `origin` | `NEED` \| `MINIMUM_STOCK` \| `PRESENCE` \| `MANUAL` | |
| `food_id` / `group_id` | `Id` ? | au plus un ; aucun pour un article libre |
| `label` | texte ? | article libre |
| `needed_amount` | `Amount` ? | besoin non couvert, mesure de stock |
| `product_id` | `Id` ? | produit proposé, puis acheté |
| `packages` | entier ? | nombre de conditionnements (R-COU-21) |
| `bought_amount` | `Amount` ? | quantité achetée |
| `aisle_id` | `Id` ? | |
| `checked` | booléen | |
| `price_paid` | `Money` ? | |
| `updated_on_device_at` | `Timestamp` | horodatage du dernier geste (R-COU-42) |

### `PriceObservation`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `product_id` / `food_id` | `Id` ? | exactement un des deux |
| `store_id` | `Id` ? | FK `Store` |
| `date` | `Date` | |
| `amount` | `Amount` | > 0, mesure de stock |
| `price_paid` | `Money` | ≥ 0 |
| `unit_price` | `Money` | centimes par kg, L ou pièce (R-COU-60) |
| `shopping_item_id` | `Id` ? | FK `ShoppingItem` |

## Anti-gaspillage — spec 07

### `Setting`

Réglages généraux de l'application (seuil `SOON`, durée des restes, début de semaine, fuseau du foyer,
heures de notification, poids de génération…). Ceux de l'IA (ADR-0014) : `ai.enabled`, `ai.provider`
(`MISTRAL` \| `OPENAI`), `ai.base_url`, `ai.model`, `ai.api_key` (jamais renvoyée par l'API, exclue de
l'export), `ai.monthly_limit`, `ai.new_recipes_per_plan`, et le compteur `ai.usage.AAAA-MM`. Ceux des
photos libres (R-REC-73) : `photos.enabled` et `photos.api_key` (clé Pixabay, jamais renvoyée ni exportée).

| Champ | Type | Contraintes |
|---|---|---|
| `key` | texte | PK ; liste fermée, définie dans le code avec sa valeur par défaut |
| `value` | `Json` | validé par le schéma du réglage |

### `Notification`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `kind` | `NotificationKind` | `LOT_SOON`, `LOT_EXPIRED`, `LEFTOVER_TO_FINISH`, `THAW_REMINDER`, `UNUSED_LOT`, `EXPIRY_MISSING`, `MEAL_TO_VALIDATE`, `DAILY_SUMMARY` |
| `lot_id` / `meal_id` | `Id` ? | cible de l'alerte |
| `dedup_key` | texte | unique ; empêche de renvoyer la même alerte (R-GAS-32) |
| `created_at` | `Timestamp` | |
| `pushed_at`, `read_at` | `Timestamp` ? | |

### `PushSubscription`

| Champ | Type | Contraintes |
|---|---|---|
| `id` | `Id` | PK |
| `endpoint` | texte | unique ; adresse fournie par le navigateur |
| `p256dh`, `auth` | texte | clés de chiffrement du navigateur (Web Push) |
| `device_label` | texte ? | |
| `created_at`, `last_success_at` | `Timestamp`, `Timestamp` ? | un abonnement refusé par le service (410) est supprimé |

Les clés VAPID du serveur sont dans le volume `/data`, générées au premier démarrage, jamais dans la base
exportée ni dans le dépôt.

## Lien avec Cashflow — spec 08

Foodflow ne stocke que des **identifiants** Cashflow, jamais de montant ni de libellé (ADR-0010,
R-CSH-35). L'adresse de Cashflow et le jeton sont dans la configuration du serveur, pas dans la base.

### `CashflowCategory`

| Champ | Type | Contraintes |
|---|---|---|
| `cashflow_category_id` | texte | PK ; catégorie suivie (R-CSH-10) |

### `Store` (compléments)

| Champ | Type | Contraintes |
|---|---|---|
| `cashflow_payee_id` | texte ? | tiers Cashflow (R-CSH-20) |
| `cashflow_payee_group_id` | texte ? | groupe de tiers Cashflow ; au plus un des deux |

### `TripPayment`

| Champ | Type | Contraintes |
|---|---|---|
| `trip_id` | `Id` | FK `ShoppingTrip` |
| `cashflow_transaction_id` | texte | unique : une transaction pour une seule session (R-CSH-34) |
| `confirmed_at` | `Timestamp` | |
