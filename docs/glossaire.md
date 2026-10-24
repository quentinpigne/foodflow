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
| Groupe nutritionnel | `NutritionGroup` | Groupe des repères de consommation (légumes, fruits, féculents, légumineuses…) attaché à une famille ; détaillé en spec `NUT`. |

## Stock (spec 02)

| Terme | Identifiant | Définition |
|---|---|---|
| Emplacement | `Location` | Lieu de rangement, de nature `PANTRY`, `FRIDGE` ou `FREEZER`. |
| Lot | `StockLot` | Quantité d'un produit, d'un aliment ou de restes, rangée au même endroit avec la même péremption. |
| Lot de préparation | `prepared lot` | Lot contenant une recette cuisinée (en portions ou en quantité) : restes d'un repas ou préparation à l'avance. |
| Restes | `leftovers` | Ce qui reste d'un repas cuisiné, remis en stock comme lot de préparation. |
| Mouvement | `StockMovement` | Entrée, sortie ou constat daté sur un lot. Jamais modifié : on l'annule par un mouvement inverse. |
| Type de mouvement | `MovementKind` | `PURCHASE`, `COOKED`, `LEFTOVER`, `PREPARED`, `WASTE`, `MOVE`, `ADJUSTMENT`, `INVENTORY`, `OPENED`. |
| Inventaire | `INVENTORY` | Mouvement qui constate la quantité réelle d'un lot et conserve l'écart avec la quantité calculée. |
| Mode de suivi | `TrackingMode` | `QUANTITY` (quantité exacte) ou `PRESENCE` (état seulement), réglé par aliment. |
| État de présence | `PresenceState` | `AVAILABLE`, `LOW`, `OUT`. |
| Lot épuisé | `depleted` | Lot de quantité nulle ou d'état `OUT`. |
| Nature de la péremption | `ExpiryKind` | `USE_BY` (DLC, « à consommer jusqu'au »), `BEST_BEFORE` (DDM, « de préférence avant »), `NONE`. |
| Date limite effective | `effective expiry` | Date limite d'un lot compte tenu de son ouverture, de sa congélation et de sa décongélation. |
| Durée de conservation | `shelf life` | Nombre de jours de conservation d'un aliment : sans date, après ouverture, au congélateur, après décongélation. |
| État de péremption | `ExpiryStatus` | `OK`, `SOON`, `EXPIRED`, `PAST_BEST_BEFORE`. |
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

## Recettes et planification (ADR-0011, specs à venir)

| Terme | Identifiant | Définition |
|---|---|---|
| Temps actif | `active time` | Temps pendant lequel on travaille à une recette (éplucher, saisir, assembler). |
| Temps passif | `passive time` | Temps d'attente d'une recette (cuisson au four, repos, marinade), pendant lequel on est libre. |
| Sous-recette | `sub-recipe` | Recette utilisée comme ingrédient d'une autre (sauce tomate maison, pâte brisée). |
| Temps de finition | `finishing time` | Temps actif restant le jour du repas pour une recette préparée à l'avance (réchauffer, assembler). |
| Semaine type | `WeekTemplate` | Créneaux de repas, temps disponible et heure limite par jour et créneau, sessions de préparation. |
| Créneau de repas | `MealSlot` | Repas de la journée (petit-déjeuner, déjeuner, goûter, dîner), paramétrable. |
| Temps disponible | `available time` | Temps actif dont on dispose pour un créneau ou une session de préparation. |
| Session de préparation | `PrepSession` | Moment dédié à cuisiner à l'avance pour plusieurs repas de la semaine (batch cooking). |
| Préparation à l'avance | `make-ahead` | Recette ou sous-recette cuisinée dans une session de préparation pour un repas ultérieur. |

## Transverse

| Terme | Identifiant | Définition |
|---|---|---|
| Validation | `validation` | Action explicite qui transforme une proposition (courses, repas cuisiné, import) en données (ADR-0005). |
| Annulation | `reversal` | Mouvement inverse qui neutralise un mouvement validé. |
| Brouillon | `draft` | Proposition en attente de validation (recette importée, produit Open Food Facts). |
