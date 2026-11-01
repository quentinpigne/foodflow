# Spec 01 — Aliments

Statut : **Validée** (2026-10-25)
Modèle : [FoodGroup, Food, Product, Nutrient, Unit](../modele-de-donnees.md#aliments--spec-01)
Décisions : ADR-0001 (entiers), ADR-0003 (familles, aliments, produits), ADR-0007 (services tiers)

## Concepts

Le référentiel d'aliments a trois niveaux (ADR-0003) :

```
Famille       Produits laitiers > Laits                      groupe nutritionnel : PRODUITS_LAITIERS
  └─ Aliment  « Lait demi-écrémé »   Ciqual 19041            mesure de stock : VOLUME
       │      nutriments /100 g · densité 1,03 g/mL · suivi : QUANTITY
       ├─ Produit  « Lactel demi-écrémé 1 L »    EAN 3428271940014   conditionnement 1 L
       └─ Produit  « Lait ½ écrémé Carrefour 6×1 L »  EAN 3560070…   conditionnement 6 L
```

| Niveau | Sert à | Source |
|---|---|---|
| **Famille** (`FoodGroup`) | Classer, hériter des réglages par défaut, exprimer « au choix » dans une recette, viser des goûts | Ciqual (arborescence initiale), modifiable |
| **Aliment** (`Food`) | Recettes, besoins, liste de courses, valeurs nutritionnelles | Ciqual ou saisie |
| **Produit** (`Product`) | Stock, scan en magasin, prix, valeurs de l'étiquette | Open Food Facts ou saisie |

### Mesures et unités

Une quantité appartient à l'une de trois **mesures** (`Measure`), chacune stockée en entier (ADR-0001) :

| Mesure | Unité de stockage | Unités de saisie courantes |
|---|---|---|
| `MASS` | mg | mg, g, kg, livre (500 g) |
| `VOLUME` | µL | mL, cL, dL, L, c. à café (5 mL), c. à soupe (15 mL), verre (200 mL), tasse (250 mL) |
| `COUNT` | millième de pièce | pièce |

En plus de ces unités générales, un aliment peut avoir ses **unités propres** (`FoodUnit`) :
gousse d'ail ≈ 5 g, sachet de levure = 11 g, tranche de jambon ≈ 45 g, pincée de sel ≈ 0,4 g.

Chaque aliment a une **mesure de stock** : celle dans laquelle on compte son stock, ses besoins et sa
liste de courses (œufs : `COUNT`, lait : `VOLUME`, farine : `MASS`).

Les conversions d'une mesure à l'autre passent par deux données propres à l'aliment :

```
           densité (mg par mL)
  VOLUME ◄──────────────────────► MASS ◄──────────────────────► COUNT
                                         poids d'une pièce (mg)
```

### Valeurs nutritionnelles

Les nutriments forment une **liste de référence** (`Nutrient`), chacun avec son unité de stockage :
énergie (cal, millième de kcal), protéines, glucides, sucres, lipides, acides gras saturés, fibres, sel
(mg), puis les autres composants de Ciqual (minéraux, vitamines : mg ou µg). Toutes les valeurs sont
stockées **pour 100 g** ou **pour 100 mL** (la **base nutritionnelle** de l'aliment ou du produit).

## Règles

### Familles

- **R-ALI-1** — Les familles forment un arbre : une famille a au plus un parent, et aucun cycle n'est
  permis. Un aliment appartient à **exactement une** famille.
- **R-ALI-2** — Une famille porte des **réglages par défaut**, hérités par ses sous-familles et ses
  aliments tant qu'ils ne les remplacent pas : groupe nutritionnel et taille de portion (spec 04), mode de suivi du stock,
  mesure de stock, durées de conservation (spec 02). La valeur effective d'un réglage est celle de
  l'élément le plus spécifique qui le définit.
- **R-ALI-3** — Une famille qui contient des aliments, des sous-familles, ou que vise un ingrédient de
  recette ou une préférence, ne peut pas être supprimée ; elle peut être fusionnée dans une autre
  (tout ce qui la vise est déplacé).
- **R-ALI-4** — Une famille peut désigner un **aliment de référence**, choisi parmi ses aliments ou ceux
  de ses sous-familles (« Fromage râpé » → « Emmental »). Il fournit les valeurs nutritionnelles et le
  prix d'un ingrédient de recette qui vise la famille, marqués estimés (R-REC-24).

### Aliments

- **R-ALI-10** — Un aliment a un nom unique (comparaison sans accents ni casse) et peut avoir des
  **alias**, utilisés par la recherche et par le rattachement des ingrédients importés.
- **R-ALI-11** — Un aliment issu de Ciqual porte son code Ciqual (unique). Ses valeurs nutritionnelles
  importées ne sont pas modifiables directement : une valeur saisie à la main les **remplace** et reste
  distinguée de la valeur Ciqual, qu'on peut rétablir.
- **R-ALI-12** — Les quantités d'un aliment s'entendent **telles qu'achetées et cuisinées** (riz cru, et
  non riz cuit). Un aliment cuit (« riz cuit ») est un autre aliment ; aucune conversion cru ↔ cuit
  n'est faite en V1.
- **R-ALI-13** — Deux aliments peuvent être **fusionnés** : tout ce qui visait l'aliment absorbé
  (produits, lots, ingrédients, préférences, alias) est reporté sur l'aliment conservé, et son nom devient
  un alias.

### Valeurs nutritionnelles

- **R-ALI-20** — Une valeur nutritionnelle est **connue** (éventuellement nulle) ou **inconnue**
  (absente). Une valeur inconnue n'est jamais traitée comme zéro : tout total qui l'inclut est
  **incomplet** et l'indique (« 2 ingrédients sans valeur de fibres »).
- **R-ALI-21** — Import Ciqual : une valeur « - » est inconnue ; « traces » est importée comme 0 et
  « < x » comme x, toutes deux marquées **estimées**.
- **R-ALI-22** — Les valeurs d'un produit, nutriment par nutriment, **priment** sur celles de son
  aliment ; un nutriment inconnu pour le produit prend la valeur de l'aliment. Un produit rattaché
  seulement à une famille n'a que ses propres valeurs.
- **R-ALI-23** — Passer d'une base pour 100 mL à une base pour 100 g (ou l'inverse) exige la densité ;
  sans elle, le calcul est impossible et le total est incomplet (R-ALI-20).
- **R-ALI-24** — Énergie : si elle est inconnue mais que protéines, glucides, lipides et fibres sont
  connus, elle est **calculée** avec les coefficients réglementaires (4, 4, 9, 2 kcal/g) et marquée
  estimée.

### Unités et conversions

- **R-ALI-30** — Toute quantité saisie (recette, stock, liste de courses) est convertie dans la mesure
  de stock de l'aliment au moment de son enregistrement ; la quantité et l'unité saisies sont conservées
  pour l'affichage.
- **R-ALI-31** — Une conversion entre mesures n'utilise que la densité ou le poids d'une pièce de
  l'aliment (ou d'une unité propre). S'ils manquent, la conversion est **impossible**, et l'interface
  propose de les renseigner ; elle ne suppose jamais une densité de 1.
- **R-ALI-32** — Les unités propres d'un aliment sont exprimées dans une mesure (la gousse en `MASS`) et
  se convertissent ensuite comme toute quantité de cette mesure.
- **R-ALI-33** — Les conversions se calculent en décimal exact et ne sont arrondies qu'au stockage, à
  l'unité de stockage la plus proche (ADR-0001).

### Produits

- **R-ALI-40** — Un produit est rattaché à un aliment ou, à défaut, à une famille (ADR-0003). Un produit
  rattaché à une famille ne peut satisfaire que les ingrédients qui visent cette famille ou l'un de ses
  ancêtres.
- **R-ALI-41** — Un code-barres (EAN-8, EAN-13, UPC-A ; clé de contrôle vérifiée) désigne au plus un
  produit. Un produit sans code-barres est permis (vrac, marché).
- **R-ALI-42** — Le **conditionnement** d'un produit est une quantité dans la mesure de stock de son
  aliment (1 L de lait, 6 œufs, 500 g de pâtes). Un produit vendu en lot (6 × 1 L) peut déclarer son
  nombre d'unités, pour entrer en stock en un ou plusieurs lots (spec 02).
- **R-ALI-43** — Un aliment peut désigner un **produit habituel**, proposé en premier sur la liste de
  courses.

### Import Ciqual

- **R-ALI-50** — L'import Ciqual est une commande du binaire, lancée sur un fichier de la table
  officielle (XML) ; il est idempotent. Réimporter une version plus récente met à jour les aliments par
  code Ciqual sans modifier les valeurs saisies à la main (R-ALI-11), ni les noms et familles modifiés
  par l'utilisateur.
- **R-ALI-51** — Les groupes, sous-groupes et sous-sous-groupes de Ciqual créent l'arborescence initiale
  des familles, chacune repérée par son code Ciqual. Une famille déplacée ou renommée par l'utilisateur
  le reste après un réimport.
- **R-ALI-52** — Les aliments Ciqual sont importés **masqués** par défaut sauf une sélection courante
  (environ 800 aliments bruts usuels, liste `seed/ciqual_selection.json` versionnée dans le dépôt) ; la recherche peut inclure les
  aliments masqués, et en utiliser un l'affiche.

### Open Food Facts

- **R-ALI-60** — Scanner un code-barres inconnu, si Open Food Facts est activé (ADR-0007), interroge
  Open Food Facts et produit un **brouillon de produit** : nom, marque, conditionnement, valeurs pour
  100 g ou 100 mL, catégories. Le brouillon propose un aliment ou une famille de rattachement ;
  le produit n'est créé qu'après validation (ADR-0005).
- **R-ALI-61** — Un produit créé depuis Open Food Facts garde la réponse brute et sa date ; il ne
  déclenche plus d'appel, sauf demande explicite d'actualisation, qui ne remplace pas les valeurs
  modifiées à la main.
- **R-ALI-62** — Le rattachement validé d'une catégorie Open Food Facts à un aliment ou une famille est
  mémorisé et proposé pour les produits suivants de la même catégorie.

## Features

| ID | Priorité | Feature |
|---|---|---|
| ALI-01 | MVP | Familles : arborescence, réglages par défaut hérités, fusion |
| ALI-02 | MVP | Aliments saisis à la main : nom, alias, famille, mesure de stock, nutriments, densité, poids d'une pièce |
| ALI-03 | MVP | Import de la table Ciqual (commande), sélection d'aliments courants affichés |
| ALI-04 | MVP | Unités générales et unités propres, conversions entre mesures |
| ALI-05 | MVP | Produits saisis à la main, code-barres, conditionnement, produit habituel |
| ALI-06 | MVP | Recherche d'aliments et de produits, sans accents ni casse, par alias, code-barres |
| ALI-07 | V1 | Scan du code-barres avec la caméra du téléphone ou un lecteur de code-barres (R-STK-50) |
| ALI-08 | V1 | Brouillon de produit depuis Open Food Facts, rattachement mémorisé par catégorie |
| ALI-09 | V1 | Fusion d'aliments |
| ALI-10 | V2 | Partie comestible (épluchures, os) : valeurs nutritionnelles rapportées à la quantité achetée |
| ALI-11 | V2 | Conversion cru ↔ cuit (rendement de cuisson) |
| ALI-12 | V2 | Affichage des micronutriments dans les bilans |

### Critères d'acceptation

- Étant donné l'aliment « Lait demi-écrémé » de mesure `VOLUME` et de densité 1,03 g/mL, quand une
  recette demande 206 g de lait, alors le besoin est de 200 mL (R-ALI-30, R-ALI-31).
- Étant donné l'aliment « Farine » sans densité, quand une recette demande 2 verres de farine, alors la
  quantité est marquée non convertible et l'interface propose de renseigner la densité (R-ALI-31).
- Étant donné un produit dont les fibres sont inconnues et dont l'aliment a 2,1 g de fibres pour 100 g,
  alors le produit affiche 2,1 g de fibres, avec l'aliment comme source (R-ALI-22).
- Étant donné une recette dont un ingrédient n'a pas de valeur de sel, alors le sel total de la
  recette est affiché comme incomplet, jamais comme exact (R-ALI-20).
- Étant donné l'import de la table Ciqual 2020 puis de la table 2025, quand j'ai remplacé à la main les
  protéines d'un aliment et renommé une famille, alors ces deux modifications sont conservées
  (R-ALI-50, R-ALI-51).
