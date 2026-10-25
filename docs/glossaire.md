# Glossaire

Statut : **Brouillon** (2026-10-25)

Langage commun du projet. Chaque terme métier a **un seul** nom français (documentation) et
**un seul** identifiant anglais (code). En cas de doute, ce fichier fait foi.

## Aliments (spec 01)

| Terme | Identifiant | Définition |
|---|---|---|
| Famille d'aliments | `FoodGroup` | Nœud de l'arborescence de classement (« Produits laitiers > Laits »). Porte des réglages par défaut hérités. |
| Aliment | `Food` | Aliment générique (« Lait demi-écrémé ») : valeurs nutritionnelles, conversions, mode de suivi. Ce que demandent les recettes. |
| Produit | `Product` | Article du commerce (« Lactel demi-écrémé 1 L »), avec code-barres et conditionnement. Toujours rattaché à un aliment ou à une famille. |
| Code-barres | `barcode` | EAN-8, EAN-13 ou UPC-A d'un produit. |
| Conditionnement | `package quantity` | Quantité d'un produit telle que vendue (1 L, 6 œufs). |
| Produit habituel | `preferred product` | Produit proposé en premier pour un aliment sur la liste de courses. |
| Alias | `FoodAlias` | Autre nom d'un aliment, pour la recherche et le rattachement des ingrédients importés. |
| Mesure | `Measure` | Grandeur d'une quantité : `MASS` (mg), `VOLUME` (µL), `COUNT` (millième de pièce). |
| Mesure de stock | `stock measure` | Mesure dans laquelle on compte le stock, les besoins et les courses d'un aliment. |
| Unité | `Unit` | Unité de saisie générale (g, L, c. à soupe, pièce), définie dans une mesure. |
| Unité propre | `FoodUnit` | Unité propre à un aliment (gousse d'ail, sachet de levure), définie par une quantité dans une mesure. |
| Densité | `density` | Masse d'un mL d'un aliment, en mg ; permet de passer du volume à la masse. |
| Poids d'une pièce | `piece weight` | Masse moyenne d'une pièce d'un aliment, en mg ; permet de passer du nombre à la masse. |
| Nutriment | `Nutrient` | Composant de la liste de référence (énergie, protéines, sel, vitamine C…), avec son unité de stockage. |
| Base nutritionnelle | `nutrient basis` | Quantité de référence des valeurs nutritionnelles : `PER_100_G` ou `PER_100_ML`. |
| Valeur estimée | `estimated` | Valeur nutritionnelle approchée (« traces », « < x », énergie calculée). |
| Total incomplet | `incomplete` | Total (nutriment, coût) dont au moins une composante est inconnue ou non convertible. |
| Groupe nutritionnel | `NutritionGroup` | Groupe des repères de consommation (légumes, fruits, féculents, légumineuses…) attaché à une famille ; détaillé en spec 04. |
| Taille de portion | `portion size` | Quantité d'un aliment qui compte pour une portion de son groupe nutritionnel (80 g de légumes). |

## Stock (spec 02)

| Terme | Identifiant | Définition |
|---|---|---|
| Emplacement | `Location` | Lieu de rangement, de nature `PANTRY`, `FRIDGE` ou `FREEZER`. |
| Lot | `StockLot` | Quantité d'un produit, d'un aliment ou de restes, rangée au même endroit avec la même péremption. |
| Lot de préparation | `prepared lot` | Lot contenant une recette cuisinée (en portions ou en quantité) : restes d'un repas ou préparation à l'avance. |
| Restes | `leftovers` | Ce qui reste d'un repas cuisiné, remis en stock comme lot de préparation ou comme lot de son aliment de reste (ADR-0012). |
| Aliment de reste | `leftover food` | Aliment que devient le reste d'une recette pour être réutilisé (poulet rôti → « Poulet, viande rôtie »). |
| Reste prévu | `planned leftovers` | Portions cuisinées en plus des convives, à affecter à un repas ultérieur. |
| Mouvement | `StockMovement` | Entrée, sortie ou constat daté sur un lot. Jamais modifié : on l'annule par un mouvement inverse. |
| Type de mouvement | `MovementKind` | `PURCHASE`, `COOKED`, `LEFTOVER`, `PREPARED`, `WASTE`, `MOVE`, `ADJUSTMENT`, `INVENTORY`, `OPENED`, `CHECKED`. |
| Inventaire | `INVENTORY` | Mouvement qui constate la quantité réelle d'un lot et conserve l'écart avec la quantité calculée. |
| Mode de suivi | `TrackingMode` | `QUANTITY` (quantité exacte) ou `PRESENCE` (état seulement), réglé par aliment. |
| État de présence | `PresenceState` | `AVAILABLE`, `LOW`, `OUT`. |
| Lot épuisé | `depleted` | Lot de quantité nulle ou d'état `OUT`. |
| Nature de la péremption | `ExpiryKind` | `USE_BY` (DLC, « à consommer jusqu'au »), `BEST_BEFORE` (DDM, « de préférence avant »), `NONE`. |
| Date limite effective | `effective expiry` | Date limite d'un lot compte tenu de son ouverture, de sa congélation et de sa décongélation. |
| Durée de conservation | `shelf life` | Nombre de jours de conservation d'un aliment : sans date, après ouverture, au congélateur, après décongélation. |
| État de péremption | `ExpiryStatus` | `OK`, `SOON`, `EXPIRED`, `PAST_BEST_BEFORE`. Une date dépassée n'impose pas de jeter : l'utilisateur décide. |
| Jugé encore bon | `CHECKED` | Décision de garder un lot dont la date est dépassée, avec une nouvelle date limite. |
| Premier périmé, premier sorti | `FEFO` | Règle de choix des lots lors d'une sortie : la date limite effective la plus proche d'abord. |
| Stock minimum | `minimum stock` | Seuil en dessous duquel un aliment est proposé pour les courses. |

## Foyer (ADR-0004)

| Terme | Identifiant | Définition |
|---|---|---|
| Membre du foyer | `Member` | Personne du foyer : nom, coefficient d'équivalent adulte, présence par défaut, préférences. N'est pas un utilisateur. |
| Équivalent adulte | `adult equivalent` | Coefficient d'un convive par rapport à un adulte (1,000) ; sert aux portions et à l'objectif nutritionnel. |
| Convive | `Diner` | Personne présente à un repas : membre, ou invité anonyme avec son coefficient. |
| Invité | `guest` | Convive ponctuel, hors du foyer. |
| Préférence | `Preference` | Goût d'un membre pour un aliment, une famille ou une recette : `LOVES`, `LIKES`, `DISLIKES`, `REFUSES`. |
| Présence par défaut | `MemberPresence` | Présence habituelle d'un membre à un créneau, par jour de la semaine. |

## Recettes (spec 03) et planification (ADR-0011)

| Terme | Identifiant | Définition |
|---|---|---|
| Temps actif | `active time` | Temps pendant lequel on travaille à une recette (éplucher, saisir, assembler). |
| Temps passif | `passive time` | Temps d'attente d'une recette (cuisson au four, repos, marinade), pendant lequel on est libre. |
| Recette | `Recipe` | Ingrédients, étapes, rendement, temps et préparation à l'avance d'un plat. |
| Catégorie de recette | `RecipeKind` | `MAIN`, `SIDE`, `STARTER`, `DESSERT`, `BREAKFAST`, `SNACK`, `BASE` (préparation de base, souvent sous-recette). |
| Rendement | `yield` | Ce que produit une recette : des portions adulte, ou une quantité (900 g de sauce). |
| Portion adulte | `adult portion` | Portion pour un équivalent adulte ; unité du rendement en portions. |
| Mise à l'échelle | `scaling` | Calcul des quantités d'une recette pour un nombre d'équivalents adulte ou une quantité voulue. |
| Ingrédient | `RecipeIngredient` | Ligne de recette visant un aliment, une famille ou une sous-recette, avec une quantité ou « à volonté ». |
| À volonté | `to taste` | Ingrédient sans quantité (sel, poivre) : hors calculs, seulement « à avoir ». |
| Ingrédient fixe | `fixed` | Ingrédient non mis à l'échelle (une feuille de laurier). |
| Aliment de référence | `reference food` | Aliment d'une famille qui fournit valeurs et prix d'un ingrédient « au choix ». |
| Étape | `RecipeStep` | Instruction de recette, avec un minuteur facultatif. |
| Brouillon de recette | `RecipeDraft` | Recette importée en attente de revue. |
| Rattachement mémorisé | `IngredientMapping` | Lien validé entre un nom d'ingrédient importé et un aliment, une famille ou une recette. |
| Confiance | `MatchConfidence` | Qualité d'un rattachement proposé : `HIGH`, `MEDIUM`, `LOW`, `NONE`. |
| Sous-recette | `sub-recipe` | Recette utilisée comme ingrédient d'une autre (sauce tomate maison, pâte brisée). |
| Temps de finition | `finishing time` | Temps actif restant le jour du repas pour une recette préparée à l'avance (réchauffer, assembler). |
| Semaine type | `WeekTemplate` | Créneaux de repas, temps disponible et heure limite par jour et créneau. Les sessions de préparation n'en font pas partie. |
| Créneau de repas | `MealSlot` | Repas de la journée (petit-déjeuner, déjeuner, goûter, dîner), paramétrable. |
| Temps disponible | `available time` | Temps actif dont on dispose pour un créneau ou une session de préparation. |
| Session de préparation | `PrepSession` | Moment dédié à cuisiner à l'avance pour plusieurs repas (batch cooking), ajouté à une semaine donnée. |
| Préparation à l'avance | `make-ahead` | Recette ou sous-recette cuisinée dans une session de préparation pour un repas ultérieur. |

## Objectifs nutritionnels (spec 04)

| Terme | Identifiant | Définition |
|---|---|---|
| Objectif nutritionnel | `NutritionGoal` | Ensemble de critères du foyer, par équivalent adulte, versionné par date de début. |
| Version d'objectif | `NutritionGoal` (`valid_from`) | Critères en vigueur à partir d'une date, jusqu'à la version suivante. |
| Critère | `GoalCriterion` | Exigence mesurable : énergie, nutriment, portions, quantité ou fréquence d'un groupe, variété. |
| Importance | `CriterionImportance` | `REQUIRED` (contrainte), `PREFERRED` (à approcher), `INFO` (affiché seulement). |
| Modèle d'objectif | `GoalTemplate` | Jeu de critères prêt à copier (« Repères PNNS 2019 — adulte »). |
| Part d'énergie | `energy share` | Part de l'énergie de la journée attribuée à un créneau de repas. |
| Hors foyer | `out of home` | Repas pris à l'extérieur : il occupe son créneau sans compter dans le stock, les courses ni le bilan. |
| Bilan | `NutritionReport` | Évaluation des critères sur une période, en lecture prévue ou réelle. |
| Statut d'un critère | `CriterionStatus` | `MET`, `BELOW`, `ABOVE`, `INCOMPLETE`, `NOT_APPLICABLE`. |

## Planification (spec 05)

| Terme | Identifiant | Définition |
|---|---|---|
| Repas | `Meal` | Une date, un créneau, des convives et des éléments ; statut `PLANNED`, `DONE`, `SKIPPED` ou `OUT_OF_HOME`. |
| Élément de repas | `MealItem` | Recette cuisinée sur place ou préparée, part d'un lot, ou aliment, dans un repas. |
| Composition type | `SlotRole` | Rôles d'un repas pour un créneau (plat, accompagnement, laitage ou fruit), obligatoires ou facultatifs. |
| Repas habituel | `HabitualMeal` | Repas fixe défini par équivalent adulte, repris par défaut sur un créneau (petit-déjeuner, goûter). |
| Portions cuisinées | `cooked portions` | Portions préparées pour un élément ; au-delà des équivalents adulte, la différence est un reste prévu. |
| Repas verrouillé | `locked` | Repas que la génération ne modifie pas. |
| Consommation réelle | `MealIngredientUse` | Ce qui a été utilisé et mangé, enregistré à la validation ; base du bilan réel. |
| Phase | `RecipePhase` | Partie active ou passive d'une recette, avec sa durée et les équipements qu'elle occupe (ADR-0015). |
| Équipement | `Equipment` | Appareil de la cuisine (four, feux, robot) et sa capacité. |
| Planning | `schedule` | Ordre et horaires des phases d'une session ou d'un repas, calculés par l'ordonnancement (ADR-0015). |
| Cuisiniers | `cooks` | Nombre de personnes qui cuisinent pendant une session ; une phase active en occupe une. |
| Préparation | `PrepItem` | Recette cuisinée dans une session de préparation, pour un ou plusieurs repas. |
| Signalement | `warning` | Anomalie non bloquante d'un repas, d'une journée ou d'une semaine. |
| Génération | `GenerationRun` | Proposition de menus, déterministe (ADR-0013) ou par IA (ADR-0014), en attente d'acceptation. |
| Graine | `seed` | Nombre qui fixe les départages d'une génération déterministe ; même graine, même proposition. |
| Réglages de génération | `generation settings` | Poids du score, nombre d'itérations, plafond de coût. |

## Courses (spec 06)

| Terme | Identifiant | Définition |
|---|---|---|
| Besoin | `need` | Quantité (ou présence) d'un aliment demandée à une date par un repas, une préparation, un repas habituel ou le stock minimum. |
| Couverture | `coverage` | Part d'un besoin assurée par le stock utilisable à sa date. |
| Session de courses | `ShoppingTrip` | Passage en magasin à une date ; couvre les besoins jusqu'à la session suivante. |
| Liste de courses | `shopping list` | Ce qu'il faut acheter pour une session : calculée tant qu'elle est prévue, figée au départ en magasin. |
| Article | `ShoppingItem` | Ligne de la liste : aliment et produit proposé, ou article libre. |
| Article libre | `free item` | Article en texte, hors stock (papier cuisson). |
| Ajustement | `ShoppingAdjustment` | Ajout, quantité imposée ou retrait, conservé au recalcul de la liste. |
| Rayon | `Aisle` | Classement d'un article dans le parcours du magasin. |
| Magasin | `Store` | Lieu d'achat, avec son ordre des rayons. |
| Relevé de prix | `PriceObservation` | Prix payé pour une quantité d'un produit, à une date, dans un magasin. |
| Prix unitaire | `unit price` | Prix au kg, au L ou à la pièce. |
| Prix de référence | `reference price` | Prix unitaire retenu pour estimer coûts et totaux (dernier relevé du produit habituel, sinon d'un produit de l'aliment). |
| Total estimé | `estimated total` | Somme des articles au prix de référence. |
| Total payé | `total paid` | Montant réellement payé pour une session ; sert au rapprochement avec Cashflow. |

## Transverse

| Terme | Identifiant | Définition |
|---|---|---|
| Validation | `validation` | Action explicite qui transforme une proposition (courses, repas cuisiné, import) en données (ADR-0005). |
| Annulation | `reversal` | Mouvement inverse qui neutralise un mouvement validé. |
| Brouillon | `draft` | Proposition en attente de validation (recette importée, produit Open Food Facts). |
