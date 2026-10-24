# Spec 02 — Stock

Statut : **Validée** (2026-10-25)
Modèle : [Location, StockLot, StockMovement](../modele-de-donnees.md#stock--spec-02)
Décisions : ADR-0001 (entiers), ADR-0002 (journal de mouvements), ADR-0003 (mode de suivi),
ADR-0005 (validation explicite)

## Concepts

```
Emplacement « Frigo » (FRIDGE)
 ├─ Lot  Lactel demi-écrémé 1 L      péremption 02/11 (DLC)   ouvert le 24/10 → à finir avant le 27/10
 │    ├─ +1 000 mL  PURCHASE   20/10  courses du 20/10
 │    ├─   −250 mL  COOKED     22/10  dîner du 22/10 (crêpes)
 │    └─   −200 mL  COOKED     24/10  petit-déjeuner du 24/10                 quantité : 550 mL
 ├─ Lot  Reste « Lasagnes »   2,5 portions               à finir avant le 27/10   (reste d'un repas)
 └─ Lot  Moutarde de Dijon     suivi en présence : LOW
```

- **Emplacement** (`Location`) : placard, frigo, congélateur, cave… Il a une nature (`PANTRY`, `FRIDGE`,
  `FREEZER`) qui détermine les durées de conservation qui s'appliquent.
- **Lot** (`StockLot`) : ce qu'on a acheté ou préparé ensemble, au même endroit, avec la même péremption.
  Il contient **un produit, un aliment** (vrac, marché) **ou une préparation** : une recette cuisinée,
  en portions ou en quantité, qu'il s'agisse de restes d'un repas ou d'une préparation à l'avance
  (ADR-0011).
- **Mouvement** (`StockMovement`) : entrée, sortie ou constat sur un lot (ADR-0002). La quantité d'un lot
  n'est jamais stockée : elle se calcule.
- **Mode de suivi** (`TrackingMode`), réglé par aliment (hérité de sa famille, R-ALI-2) :
  - `QUANTITY` : la quantité exacte, dans la mesure de stock de l'aliment ;
  - `PRESENCE` : un état seulement, `AVAILABLE`, `LOW` ou `OUT` (épices, huile, condiments).

### Péremption

| Mention | Nature (`ExpiryKind`) | Une fois dépassée |
|---|---|---|
| « À consommer jusqu'au » (DLC) | `USE_BY` | Périmé : on demande s'il est encore utilisable ou à jeter (R-STK-37) |
| « À consommer de préférence avant » (DDM) | `BEST_BEFORE` | Encore consommable, signalé ; même question (R-STK-37) |
| Aucune (fruits, légumes, vrac) | `NONE` | Durée de conservation par défaut de l'aliment, si elle existe |

La **date limite effective** d'un lot tient compte de son ouverture, de sa congélation et de sa
décongélation (R-STK-30 à R-STK-34), et de ta décision de le garder malgré une date dépassée (R-STK-37).

## Règles

### Lots et quantités

- **R-STK-1** — Un lot contient exactement un produit, un aliment ou une recette (lot de préparation).
  Un reste de repas est un lot de préparation, ou un lot de l'aliment de reste de la recette (ADR-0012). Un lot de
  produit compte dans le stock de l'aliment (ou de la famille) du produit.
- **R-STK-2** — La quantité d'un lot suivi en `QUANTITY` = quantité du dernier inventaire + Σ des
  mouvements postérieurs (ADR-0002). Elle est exprimée dans la mesure de stock de l'aliment
  (en portions ou dans la mesure de la recette pour un lot de préparation).
- **R-STK-3** — La quantité d'un lot n'est jamais négative. Une sortie supérieure à la quantité calculée
  est **bornée** à celle-ci ; l'écart est enregistré sur le mouvement et le lot est signalé « à
  vérifier », ce qui invite à un inventaire.
- **R-STK-4** — Un lot dont la quantité est nulle, ou dont l'état est `OUT`, est **épuisé** : il
  disparaît des vues de stock, mais son journal est conservé.
- **R-STK-5** — Le stock d'un aliment = Σ des quantités de ses lots non épuisés, produits rattachés
  compris. Le stock d'une famille ne s'additionne qu'entre aliments de même mesure de stock ;
  les autres sont affichés à part.
- **R-STK-6** — Un produit vendu en lot (R-ALI-42) entre en stock en **un lot par unité** si on le
  demande (pack de 6 yaourts : un lot de 6, ou 6 lots si les dates diffèrent).

### Mouvements

- **R-STK-10** — Types de mouvements : `PURCHASE`, `COOKED`, `LEFTOVER`, `PREPARED`, `WASTE`, `MOVE`, `ADJUSTMENT`,
  `INVENTORY`, `OPENED`, `CHECKED` (ADR-0002). Chaque mouvement a une date, un type, une quantité signée (ou un état
  pour un lot en `PRESENCE`) et, s'il y a lieu, son origine : session de courses, repas, mouvement annulé.
- **R-STK-11** — Un mouvement validé n'est **ni modifié ni supprimé**. On l'annule par un mouvement
  inverse qui le référence ; un mouvement ne peut être annulé qu'une fois.
- **R-STK-12** — Annuler une validation (courses, repas) annule **tous** les mouvements qu'elle a créés,
  ensemble.
- **R-STK-13** — `WASTE` porte un motif : `EXPIRED`, `SPOILED`, `OTHER`. Ces sorties alimentent les
  statistiques de gaspillage (spec `GAS`).
- **R-STK-14** — Un mouvement est daté du jour de l'événement, qui peut être passé (saisie en retard) mais
  jamais futur. Aucun mouvement n'est antérieur au dernier inventaire du lot.
- **R-STK-15** — Déplacer **tout** un lot change son emplacement (un mouvement `MOVE` sur le lot).
  Déplacer **une partie** d'un lot (la moitié d'une viande au congélateur) crée un nouveau lot qui hérite
  de la péremption : un `MOVE` négatif sur le lot d'origine, un `MOVE` positif sur le nouveau, liés par
  le même `batch_id`.

### Mode de suivi

- **R-STK-20** — Un lot suit le mode de suivi de son aliment au moment de sa création.
- **R-STK-21** — Passer un aliment de `QUANTITY` à `PRESENCE` crée, pour chacun de ses lots non épuisés,
  un inventaire `AVAILABLE`. Passer de `PRESENCE` à `QUANTITY` exige un inventaire en quantité de chacun
  de ses lots non épuisés (proposé dans la foulée).
- **R-STK-22** — Pour un lot en `PRESENCE`, une sortie `COOKED` ne change pas l'état : seul l'utilisateur
  passe un lot à `LOW` ou `OUT` (lors de la validation d'un repas, ou à tout moment).

### Péremption et conservation

- **R-STK-30** — Un aliment (ou sa famille, R-ALI-2) peut définir des **durées de conservation** :
  après achat sans date (`NONE`), après ouverture, au congélateur, après décongélation.
- **R-STK-31** — Date limite effective d'un lot = la plus proche entre sa date de péremption et, s'il est
  ouvert, la date d'ouverture + la durée après ouverture. Si le lot a été **jugé encore bon**
  (R-STK-37), c'est la date fixée par ce jugement.
- **R-STK-32** — Déplacer un lot vers un emplacement `FREEZER` le **congèle** : sa date limite effective
  devient la date de congélation + la durée au congélateur ; la date d'origine est conservée.
  Une congélation après la date limite effective est refusée.
- **R-STK-33** — Sortir un lot d'un emplacement `FREEZER` le **décongèle** : date limite effective = date
  de décongélation + durée après décongélation. Un lot décongelé ne peut pas être recongelé (sauf
  après cuisson, qui crée un lot de préparation).
- **R-STK-34** — Un lot de préparation a pour date limite la date où il a été cuisiné + la conservation
  au frigo de sa recette (à défaut, réglage général : 3 jours) ; rangé au congélateur, R-STK-32
  s'applique, et seulement si la recette est congelable.
- **R-STK-35** — État de péremption d'un lot, à une date donnée : `OK` ; `SOON` si la date limite
  effective est dans moins de N jours (réglage général, 3 par défaut) ; `EXPIRED` si une date `USE_BY`
  ou une durée de conservation est dépassée ; `PAST_BEST_BEFORE` si seule une date `BEST_BEFORE` est
  dépassée.
- **R-STK-36** — Lorsqu'une sortie ne désigne pas de lot (repas, sortie rapide), elle puise dans les lots
  de l'aliment **dont la date limite effective est la plus proche** (premier périmé, premier sorti),
  les lots ouverts en premier à date égale.
- **R-STK-37** — Une date dépassée **ne signifie pas qu'on jette** : c'est l'utilisateur qui décide.
  Un lot `EXPIRED` ou `PAST_BEST_BEFORE` est présenté avec la question « encore utilisable ? » :
  - **à jeter** : un mouvement `WASTE` de motif `EXPIRED` épuise le lot ;
  - **encore bon** : un mouvement `CHECKED` enregistre le jugement et une nouvelle date limite
    (par défaut le lendemain, modifiable) ; le lot redevient utilisable jusqu'à cette date, puis la
    question se repose.

  La question est posée sur le tableau de bord, dans la vue du stock, et lors d'une sortie (repas
  cuisiné, sortie rapide) qui puiserait dans ce lot. Tant qu'elle n'a pas de réponse, le lot reste dans
  le stock mais n'est **ni proposé par la planification ni pris d'office par une sortie** (R-STK-36) :
  la sortie le propose avec la question.

### Inventaire et stock minimum

- **R-STK-40** — Un **inventaire** porte sur un emplacement ou un aliment : pour chaque lot, on constate
  la quantité (ou l'état). Il crée un mouvement `INVENTORY` qui fixe la quantité et conserve l'écart
  avec la quantité calculée.
- **R-STK-41** — Un inventaire peut créer des lots (trouvés) et épuiser des lots (absents).
- **R-STK-42** — Un aliment peut avoir un **stock minimum** (en quantité, ou « toujours en avoir » en
  `PRESENCE`). En dessous, l'aliment est proposé pour la liste de courses (spec `COU`), même sans repas
  qui le demande.

## Features

| ID | Priorité | Feature |
|---|---|---|
| STK-01 | MVP | Emplacements et leur nature |
| STK-02 | MVP | Lots : entrée manuelle (produit, aliment), péremption, mode de suivi |
| STK-03 | MVP | Mouvements : sortie, jeté avec motif, correction, annulation ; journal d'un lot |
| STK-04 | MVP | Vue du stock par emplacement, par aliment, par famille ; états de péremption |
| STK-05 | MVP | Inventaire par emplacement ou par aliment, écarts conservés |
| STK-06 | V1 | Ouverture d'un lot et durée après ouverture |
| STK-07 | V1 | Congélation et décongélation, durées par famille |
| STK-08 | V1 | Lots de préparation : restes d'un repas, préparations à l'avance (ADR-0011) |
| STK-09 | V1 | Stock minimum par aliment |
| STK-10 | V1 | Sortie rapide depuis le téléphone (scan du code-barres, premier périmé premier sorti) |
| STK-11 | V2 | Statistiques de consommation par aliment et par mois |

### Critères d'acceptation

- Étant donné un lot de 1 L de lait, des sorties de 250 mL et 200 mL, alors sa quantité est 550 mL ;
  et après un inventaire à 500 mL puis une sortie de 100 mL, elle est de 400 mL, avec un écart
  d'inventaire de −50 mL conservé (R-STK-2, R-STK-40).
- Étant donné un lot de 200 g, quand je valide une sortie de 300 g, alors 200 g sortent, le lot est
  épuisé et signalé « à vérifier » avec un écart de 100 g (R-STK-3).
- Étant donné un yaourt de DLC le 30/10 ouvert le 24/10 avec une durée après ouverture de 2 jours,
  alors sa date limite effective est le 26/10 (R-STK-31).
- Étant donné deux lots de crème, l'un à DLC le 28/10, l'autre le 05/11, quand un repas sort 20 cL de
  crème, alors elle est prise dans le lot du 28/10 (R-STK-36).
- Étant donné un lot décongelé, alors le déplacer vers le congélateur est refusé (R-STK-33).
- Étant donné un yaourt dont la DLC est dépassée d'un jour, quand je valide un repas qui en demande,
  alors la validation me demande s'il est encore utilisable ; si je réponds « encore bon », il est
  utilisé et un mouvement `CHECKED` est enregistré ; sinon, il est jeté (`WASTE`, `EXPIRED`) et la sortie
  puise dans le lot suivant (R-STK-37).
- Étant donné une validation de courses qui a créé 12 lots, quand je l'annule, alors les 12 mouvements
  `PURCHASE` sont annulés ensemble et les lots sont épuisés (R-STK-11, R-STK-12).
