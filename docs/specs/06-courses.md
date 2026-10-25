# Spec 06 — Courses

Statut : **Validée** (2026-10-25)
Modèle : [ShoppingTrip, ShoppingAdjustment, ShoppingItem, Store, Aisle, PriceObservation](../modele-de-donnees.md#courses--spec-06)
Décisions : ADR-0001 (entiers), ADR-0002 (stock), ADR-0003 (aliments et produits), ADR-0005
(validation), ADR-0009 (PWA hors connexion), ADR-0010 (Cashflow), ADR-0011 (sessions de préparation)

## Concepts

```
Courses du samedi 07/11 — Carrefour Market            couvre les besoins du 07/11 au 13/11
───────────────────────────────────────────────────────────────────────────────────────────
Fruits et légumes
  ☐ Courgettes            1,2 kg      besoin 1,2 kg (gratin lun.)                 ≈ 3,10 €
  ☐ Pommes                1 × sachet de 6   besoin 4,8 pièces (fruits du déj.)     ≈ 2,40 €
Crèmerie
  ☐ Lait demi-écrémé      2 × Lactel 1 L   besoin 1,4 L · stock 0,3 L             ≈ 2,30 €
  ☐ Crème fraîche         1 × 20 cL   stock minimum                                ≈ 1,20 €
Épicerie
  ☐ Moutarde              1 pot       en présence : épuisée
Autres
  ☐ Papier cuisson        (article libre, hors stock)
───────────────────────────────────────────────────────────────────────────────────────────
Total estimé ≈ 48,60 € (incomplet : 2 articles sans prix connu) · reste du budget courses : 212 € (Cashflow)
```

- **Besoin** (`need`) : quantité d'un aliment demandée à une date, par un repas planifié, une préparation
  d'une session, un repas habituel ou le stock minimum.
- **Session de courses** (`ShoppingTrip`) : un passage en magasin, à une date, dans un magasin, avec un
  statut (`PLANNED`, `IN_PROGRESS`, `DONE`, `CANCELLED`). Elle couvre les besoins jusqu'à la session de
  courses suivante.
- **Liste de courses** : ce qu'il faut acheter pour une session. Tant que la session est `PLANNED`, elle
  est **calculée** à chaque affichage, plus les **ajustements** de l'utilisateur ; au départ en magasin,
  elle est **figée** en articles (`ShoppingItem`).
- **Article** : un aliment (avec un produit proposé), ou un **article libre** (texte, hors stock : papier
  cuisson, éponges).
- **Rayon** (`Aisle`) et **magasin** (`Store`) : classement de la liste dans l'ordre du parcours.
- **Relevé de prix** (`PriceObservation`) : prix payé pour un produit, à une date, dans un magasin.

## Règles

### Besoins

- **R-COU-1** — Les besoins d'une période sont, pour chaque aliment :
  - les ingrédients des éléments des repas `PLANNED` (recettes mises à l'échelle des portions cuisinées,
    R-REC-12), datés du jour du repas ;
  - les ingrédients des préparations des sessions de préparation `PLANNED`, datés **du jour de la
    session** (ADR-0011) ;
  - les aliments des repas habituels et des éléments « aliment » des repas ;
  - le **stock minimum** (R-STK-42), daté du début de la période.

  Les éléments servis par un lot (préparation, restes) ne créent pas de besoin : le lot les couvre.
- **R-COU-2** — Une sous-recette se développe en ses ingrédients, sauf si un lot de préparation de cette
  sous-recette la couvre (R-REC-32).
- **R-COU-3** — Un ingrédient **à volonté** crée un besoin de **présence** (en avoir), pas de quantité
  (R-REC-22). Un ingrédient **facultatif** ne crée de besoin que s'il est demandé (R-REC-23).
- **R-COU-4** — Un besoin qui vise une **famille** est couvert par tout lot d'un aliment de la famille
  (R-ALI-40) ; s'il faut acheter, l'article proposé est l'aliment de référence de la famille (R-ALI-4),
  sinon la famille elle-même.

### Couverture par le stock

- **R-COU-10** — Le stock couvre les besoins **dans l'ordre chronologique**, en simulant les sorties
  (premier périmé, premier sorti, R-STK-36) : un lot ne couvre un besoin que s'il est encore utilisable
  à sa date (date limite effective ≥ date du besoin, ou jugé encore bon, R-STK-37). Ce qui reste non
  couvert est **à acheter**.
- **R-COU-11** — Un lot au congélateur couvre un besoin ; la décongélation est rappelée la veille
  (R-PLA-43).
- **R-COU-12** — Un aliment suivi en **présence** est à acheter s'il est `OUT` ; s'il est `LOW`, il est
  **proposé** (case non cochée par défaut).
- **R-COU-13** — Une quantité non convertible (R-ALI-31) ne peut pas être couverte par le calcul : l'article
  est proposé avec la mention « quantité à vérifier ».

### Liste d'une session

- **R-COU-20** — La liste d'une session couvre les besoins datés de sa date jusqu'à la veille de la
  session de courses `PLANNED` suivante (7 jours s'il n'y en a pas), plus les besoins **antérieurs non
  couverts** (urgents, signalés).
- **R-COU-21** — Les quantités à acheter sont **regroupées par aliment** et exprimées dans sa mesure de
  stock. Le produit proposé est le **produit habituel** (R-ALI-43) ; la quantité est arrondie **au
  conditionnement supérieur** (« besoin 1,4 L → 2 × 1 L »). L'excédent entrera en stock.
- **R-COU-22** — Chaque article indique **pourquoi** il est sur la liste (repas, préparation, stock
  minimum, ajout manuel) et ce que le stock couvre déjà.
- **R-COU-23** — Les **ajustements** de l'utilisateur sur une session `PLANNED` sont conservés à chaque
  recalcul : ajout d'un article (aliment ou libre), quantité imposée, article retiré. Si le besoin d'un
  article ajusté change, l'écart est signalé.
- **R-COU-24** — Un article libre n'a ni quantité convertie, ni stock, ni valeurs nutritionnelles ; il
  peut avoir un rayon et un prix.
- **R-COU-25** — Le **total estimé** = Σ des articles × prix de référence (Q20 : dernier prix payé du
  produit habituel, sinon d'un produit de l'aliment) ; il est **incomplet** si un prix manque.

### Rayons et magasins

- **R-COU-30** — Chaque famille d'aliments a un **rayon** par défaut (réglage hérité, R-ALI-2) ; un
  produit ou un aliment peut en avoir un autre. Un article libre prend le rayon choisi à sa création.
- **R-COU-31** — Un magasin a son **ordre des rayons** ; sans ordre propre, l'ordre général s'applique.
  La liste en magasin suit l'ordre des rayons du magasin de la session.

### En magasin (hors connexion, ADR-0009)

- **R-COU-40** — **Partir en courses** fige la liste : la session passe à `IN_PROGRESS` et ses articles
  sont enregistrés. La liste figée est gardée sur le téléphone (PWA) et fonctionne **sans réseau**.
- **R-COU-41** — En magasin, on peut : cocher ou décocher un article, changer le produit, la quantité,
  le nombre de conditionnements, saisir le prix payé, ajouter un article, **scanner** un code-barres
  (qui coche l'article correspondant, ou en ajoute un).
- **R-COU-42** — Hors connexion, chaque geste est horodaté par l'appareil et envoyé au retour du réseau ;
  sur un même champ d'un même article, **le dernier geste l'emporte**. Un code-barres inconnu scanné hors
  connexion est gardé et résolu au retour du réseau (R-ALI-60).
- **R-COU-43** — Les articles non cochés à la fin de la session restent à acheter : ils reviennent dans
  le calcul de la session suivante.

### Validation vers le stock (ADR-0005)

- **R-COU-50** — **Terminer les courses** propose, pour chaque article coché d'aliment ou de produit, un
  ou plusieurs **lots** (R-STK-6) préremplis : produit, quantité, emplacement (dernier emplacement connu
  du produit, sinon celui de la famille), nature et date de péremption (durée de conservation par
  défaut pour `NONE`), prix. On ajuste, puis on valide : la validation crée les lots et les mouvements
  `PURCHASE`, en un seul lot d'opérations annulable (R-STK-12).
- **R-COU-51** — Une date de péremption peut rester **à compléter** : le lot est créé et signalé, pour
  saisir la date en rangeant les courses.
- **R-COU-52** — Un produit nouveau (scanné ou saisi) doit être rattaché à un aliment ou une famille
  avant la validation (ADR-0003).
- **R-COU-53** — La session terminée enregistre le **total payé** (facultatif, prérempli par la somme des
  prix saisis). Il est nécessaire pour le rapprochement avec Cashflow (spec 08).

### Prix

- **R-COU-60** — Un prix saisi pour un article crée un **relevé de prix** : produit (ou aliment), magasin,
  date, quantité, prix payé, et **prix unitaire** (centimes par kg, par L ou par pièce), calculé en
  décimal exact et arrondi au centime (ADR-0001).
- **R-COU-61** — Le prix d'un lot = prix payé ÷ nombre de lots créés pour l'article, au prorata des
  quantités.
- **R-COU-62** — Le **prix de référence** d'un aliment (Q20) sert au coût des recettes (R-REC-53) et au
  total estimé : prix unitaire du dernier relevé du produit habituel, sinon du dernier relevé d'un
  produit de l'aliment, sinon de l'aliment lui-même ; sinon inconnu.

## Features

| ID | Priorité | Feature |
|---|---|---|
| COU-01 | MVP | Besoins d'une période et couverture par le stock, datée, premier périmé premier sorti |
| COU-02 | MVP | Session de courses et liste calculée : regroupement, produit habituel, arrondi au conditionnement, raisons |
| COU-03 | MVP | Ajustements : articles ajoutés (aliment ou libre), quantités imposées, articles retirés |
| COU-04 | MVP | Terminer les courses : lots préremplis, dates à compléter, validation et annulation |
| COU-05 | V1 | Rayons, magasins et ordre des rayons par magasin |
| COU-06 | V1 | Mode magasin hors connexion (PWA), synchronisation au retour du réseau |
| COU-07 | V1 | Scan en magasin : cocher ou ajouter un article |
| COU-08 | V1 | Prix : saisie par article, relevés, prix unitaire, prix de référence, total estimé, total payé |
| COU-09 | V1 | Aliments en présence `LOW` proposés ; stock minimum dans la liste |
| COU-10 | V2 | Historique des prix d'un produit par magasin (graphique) |
| COU-11 | V2 | Lecture du ticket de caisse (photo) pour préremplir prix et articles |

### Critères d'acceptation

- Étant donné un gratin prévu lundi qui demande 1,2 kg de courgettes et 0,4 kg en stock dont la date
  limite est dimanche, alors la liste de la session de samedi demande 1,2 kg (le lot ne sera plus
  utilisable lundi) (R-COU-10).
- Étant donné un besoin de 1,4 L de lait, 0,3 L en stock et un produit habituel de 1 L, alors la liste
  propose 2 × 1 L, avec la raison et le stock couvert (R-COU-21, R-COU-22).
- Étant donné une session de préparation dimanche et des courses prévues samedi et mercredi, alors les
  ingrédients de la session sont sur la liste de samedi (R-COU-1, R-COU-20).
- Étant donné un article dont j'ai imposé la quantité, quand j'ajoute un repas qui augmente le besoin,
  alors ma quantité est conservée et l'écart signalé (R-COU-23).
- Étant donné une liste figée sur le téléphone, quand je coche trois articles sans réseau, alors ils
  restent cochés et sont envoyés au retour du réseau (R-COU-40, R-COU-42).
- Étant donné un article de 2 × 1 L de lait coché, quand je termine les courses, alors deux lots de 1 L
  (ou un lot de 2 L, au choix) sont proposés au frigo, avec la date de péremption à saisir, et rien n'est
  créé avant la validation (R-COU-50).
- Étant donné un relevé de 3,29 € pour 1,5 kg de farine, alors le prix unitaire est de 2,19 €/kg et
  devient le prix de référence de la farine si c'est son produit habituel (R-COU-60, R-COU-62).
