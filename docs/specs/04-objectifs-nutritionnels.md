# Spec 04 — Objectifs nutritionnels

Statut : **Validée** (2026-10-25)
Modèle : [NutritionGoal, GoalCriterion, MealSlot](../modele-de-donnees.md#objectifs-nutritionnels--spec-04)
Décisions : ADR-0003 (familles), ADR-0004 (équivalent adulte, objectif du foyer), ADR-0012 (restes)

> Les repères proposés sont **indicatifs** et ne remplacent pas un avis médical ou diététique (vision,
> non-objectifs). Leurs sources sont citées dans le modèle « Repères PNNS 2019 ».

## Concepts

Un **objectif nutritionnel** (`NutritionGoal`) est un ensemble de **critères**, défini **pour un
équivalent adulte** (ADR-0004) et **versionné dans le temps** : chaque version s'applique à partir de sa
date de début, jusqu'à la version suivante.

```
Objectif « Foyer »
 ├─ version du 2026-11-02 (en vigueur)          ├─ version du 2027-01-04 (à venir)
 │   Énergie         2 000 kcal/j  ± 10 %        │   Énergie   1 900 kcal/j ± 10 %
 │   Protéines       10–20 % de l'énergie         │   …
 │   Fibres          ≥ 30 g/j
 │   Fruits et légumes  ≥ 5 portions/j
 │   Poisson         ≥ 2 fois/semaine, dont gras ≥ 1
 │   Viande rouge    ≤ 500 g/semaine
 │   Même recette    au moins 7 jours d'écart
```

### Familles de critères

| Famille | Exemple | Période |
|---|---|---|
| **Énergie** | 2 000 kcal par jour, ± 10 % | jour, moyenne de la semaine |
| **Nutriment** | Fibres ≥ 30 g/j ; lipides 35–40 % de l'énergie ; sel ≤ 6 g/j | jour, moyenne de la semaine |
| **Portions d'un groupe** | Fruits et légumes ≥ 5 portions par jour | jour ou semaine |
| **Quantité d'un groupe** | Viande rouge ≤ 500 g par semaine | semaine |
| **Fréquence d'un groupe** | Légumineuses ≥ 2 fois par semaine | semaine |
| **Variété** | Même recette : au moins 7 jours d'écart ; même famille d'aliments ≤ 3 fois par semaine | fenêtre glissante |

Chaque critère a une **importance** :

- `REQUIRED` : la génération de menus doit le respecter si une solution existe (spec `PLA`) ;
- `PREFERRED` : la génération cherche à s'en approcher ;
- `INFO` : affiché dans le bilan seulement.

### Groupes nutritionnels et portions

Chaque famille d'aliments est rattachée à un **groupe nutritionnel** (`NutritionGroup`, réglage hérité,
R-ALI-2), et chaque aliment a une **taille de portion** pour compter les portions de son groupe
(réglage hérité aussi : 80 g pour un légume, 30 g pour un fromage, 125 g pour un yaourt).

| Groupe | Code | Portion par défaut |
|---|---|---|
| Légumes | `VEGETABLES` | 80 g |
| Fruits | `FRUITS` | 80 g |
| Légumineuses | `LEGUMES` | 60 g (sèches) |
| Féculents complets | `WHOLE_GRAINS` | 60 g (crus) |
| Féculents raffinés, pommes de terre | `REFINED_STARCHES` | 60 g (crus) ; 200 g (pommes de terre) |
| Produits laitiers | `DAIRY` | 150 mL (lait), 125 g (yaourt), 30 g (fromage) |
| Volaille | `POULTRY` | 100 g |
| Viande rouge | `RED_MEAT` | 100 g |
| Charcuterie | `PROCESSED_MEAT` | 50 g |
| Poisson maigre | `FISH` | 100 g |
| Poisson gras | `FATTY_FISH` | 100 g |
| Œufs | `EGGS` | 2 pièces |
| Fruits à coque | `NUTS` | 30 g |
| Matières grasses ajoutées | `ADDED_FATS` | 10 g |
| Produits sucrés | `SWEET_PRODUCTS` | — |
| Produits salés à grignoter | `SALTY_SNACKS` | — |
| Boissons sucrées | `SUGARY_DRINKS` | — |
| Autres (condiments, épices, boissons non sucrées) | `OTHER` | — |

Le rattachement initial des groupes Ciqual aux groupes nutritionnels est livré dans le dépôt et reste
modifiable. Il suit les repères, pas le classement de Ciqual : les pommes de terre sont des féculents,
pas des légumes.

### Créneaux et répartition de l'énergie

Chaque créneau de repas (spec `PLA`) porte une **part de l'énergie de la journée** : par défaut
petit-déjeuner 25 %, déjeuner 35 %, goûter 10 %, dîner 30 %. Elle sert à fixer la cible d'un repas pour
la génération, et à ajuster la cible d'une journée dont un repas est pris hors du foyer.

## Règles

### Objectif et versions

- **R-NUT-1** — Le foyer a **un** objectif nutritionnel actif, exprimé par équivalent adulte et par jour
  (ou par semaine selon le critère). Des objectifs par membre viendront en V2 (ADR-0004).
- **R-NUT-2** — Une version a une date de début ; à une date donnée, **une seule** version est en
  vigueur : celle dont la date de début est la plus récente sans être postérieure.
- **R-NUT-3** — Une version déjà entrée en vigueur n'est **pas modifiée** : la changer crée une nouvelle
  version, qui commence aujourd'hui ou à la date choisie. Une version à venir se modifie librement.
  Le bilan d'une période passée utilise donc toujours les critères qui s'y appliquaient.
- **R-NUT-4** — Un **modèle** est un ensemble de critères prêt à l'emploi, copié dans une nouvelle
  version. L'application fournit le modèle « Repères PNNS 2019 — adulte » (tableau ci-dessous) ;
  on peut enregistrer ses propres modèles.

### Critères

- **R-NUT-10** — Un critère a une famille, une cible (nutriment, ensemble de groupes nutritionnels ou de
  familles d'aliments), une période, un minimum et/ou un maximum, et une importance.
- **R-NUT-11** — Un critère de nutriment s'exprime en **valeur absolue** (g/j) ou en **pourcentage de
  l'énergie** (lipides : 9 kcal/g ; protéines, glucides : 4 kcal/g ; fibres : 2 kcal/g).
- **R-NUT-12** — Un critère d'énergie a une cible et une **tolérance** (± %) ; il est atteint si la
  valeur est dans la tolérance.
- **R-NUT-13** — Un critère de groupe peut viser **plusieurs** groupes : « poisson 2 fois par semaine »
  vise `FISH` et `FATTY_FISH`, « dont 1 poisson gras » est un second critère sur `FATTY_FISH`.
- **R-NUT-14** — **Fréquence** : un repas compte comme une occurrence d'un critère de fréquence si, par
  équivalent adulte, il apporte au moins un **seuil** de portions du groupe (1 portion par défaut,
  réglable par critère).
- **R-NUT-15** — **Variété** : « même recette » compte les repas où la recette est cuisinée (pas ceux où
  on en mange les restes, ADR-0012) ; « même famille d'aliments » compte les repas dont la famille
  fournit au moins une portion par équivalent adulte.

### Calcul des apports

- **R-NUT-20** — Les apports d'un repas se calculent sur ce qui y est **mangé** (ADR-0012) :
  - repas planifié : quantités mises à l'échelle (R-REC-12), moins les restes prévus ;
  - repas validé : quantités réellement sorties du stock ou utilisées, moins les restes déclarés ;
  - restes mangés : portions de reste × valeurs par portion de la recette, ou quantité de l'aliment de
    reste.
- **R-NUT-21** — **Apport par équivalent adulte** d'un repas = apports du repas ÷ équivalents adulte de
  ses convives (ADR-0004). Apport d'une journée = Σ des apports par équivalent adulte de ses repas.
- **R-NUT-22** — Les portions d'un groupe = quantité des aliments du groupe ÷ leur taille de portion ;
  la quantité est convertie dans la mesure de la taille de portion (R-ALI-31), sinon le critère est
  incomplet.
- **R-NUT-23** — Un ingrédient **à volonté** (sel, poivre, huile pour la poêle) n'est pas compté
  (R-REC-22) : les critères sur le sel et les matières grasses ajoutées sont donc **sous-estimés**, ce
  que le bilan rappelle. Le modèle PNNS les met en importance `INFO`.
- **R-NUT-24** — Un total **incomplet** (R-ALI-20) rend le critère incomplet : son statut l'indique, avec
  la valeur partielle et la liste de ce qui manque. Un critère incomplet n'est jamais présenté comme
  atteint, sauf un maximum déjà dépassé par la valeur partielle.

### Journées et créneaux

- **R-NUT-30** — La part d'énergie des créneaux d'une journée type totalise 100 %.
- **R-NUT-31** — Un repas **hors foyer** (cantine, restaurant) n'est pas compté : la cible de la journée
  est réduite de la part de son créneau, pour l'énergie et pour les critères journaliers de nutriments.
  Les critères de groupe ne sont pas réduits.
- **R-NUT-32** — Un créneau **sans repas** (rien de planifié) rend la journée incomplète dans le bilan
  prévisionnel. Un repas passé ni validé ni annulé est compté pour ses valeurs prévues et signalé.

### Bilan

- **R-NUT-40** — Le **bilan** d'une période (jour, semaine, mois) donne, pour chaque critère de la version
  en vigueur : la valeur, la cible, et un **statut** : `MET`, `BELOW`, `ABOVE`, `INCOMPLETE`,
  `NOT_APPLICABLE` (aucun repas compté).
- **R-NUT-41** — Il existe en deux lectures : **prévu** (repas planifiés) et **réel** (repas validés).
- **R-NUT-42** — Les critères journaliers sont évalués chaque jour **et** en moyenne sur la semaine ;
  l'écart d'un jour n'est pas une alerte si la moyenne de la semaine est atteinte.
- **R-NUT-43** — Une période qui s'étend sur deux versions d'objectif évalue chaque jour selon sa
  version ; les critères hebdomadaires utilisent la version en vigueur le premier jour de la semaine.
- **R-NUT-44** — Il n'y a **pas de score global** : le bilan présente les critères un par un.

### Modèle « Repères PNNS 2019 — adulte »

Sources : Santé publique France, recommandations relatives à l'alimentation (2019) ; ANSES, références
nutritionnelles pour la population (apports en macronutriments, fibres).

| Critère | Valeur | Importance |
|---|---|---|
| Énergie | 2 000 kcal/j ± 10 % (à ajuster au foyer) | `PREFERRED` |
| Protéines | 10–20 % de l'énergie | `PREFERRED` |
| Lipides | 35–40 % de l'énergie | `PREFERRED` |
| Glucides | 40–55 % de l'énergie | `PREFERRED` |
| Acides gras saturés | ≤ 12 % de l'énergie | `PREFERRED` |
| Sucres | ≤ 100 g/j | `PREFERRED` |
| Fibres | ≥ 30 g/j | `PREFERRED` |
| Sel | ≤ 6 g/j | `INFO` (R-NUT-23) |
| Fruits et légumes | ≥ 5 portions/j | `PREFERRED` |
| Fruits à coque | ≥ 1 portion/j | `INFO` |
| Féculents complets | ≥ 1 portion/j | `PREFERRED` |
| Produits laitiers | 2 portions/j | `PREFERRED` |
| Légumineuses | ≥ 2 fois/semaine | `PREFERRED` |
| Poisson (maigre ou gras) | ≥ 2 fois/semaine | `PREFERRED` |
| Poisson gras | ≥ 1 fois/semaine | `PREFERRED` |
| Viande rouge | ≤ 500 g/semaine | `PREFERRED` |
| Charcuterie | ≤ 150 g/semaine | `PREFERRED` |
| Même recette | au moins 7 jours d'écart (hors restes) | `PREFERRED` |

## Features

| ID | Priorité | Feature |
|---|---|---|
| NUT-01 | MVP | Objectif versionné : critères d'énergie et de nutriments, importance |
| NUT-02 | MVP | Groupes nutritionnels et tailles de portion, hérités des familles ; rattachement initial des groupes Ciqual |
| NUT-03 | MVP | Modèle « Repères PNNS 2019 — adulte » ; modèles personnels |
| NUT-04 | MVP | Bilan prévu et réel par jour et par semaine, avec incomplétude détaillée |
| NUT-05 | V1 | Critères de groupes : portions, quantités, fréquences |
| NUT-06 | V1 | Critères de variété |
| NUT-07 | V1 | Parts d'énergie par créneau ; repas hors foyer |
| NUT-08 | V1 | Bilan mensuel et évolution sur plusieurs mois |
| NUT-09 | V2 | Objectifs par membre du foyer ; apports par membre |
| NUT-10 | V2 | Critères et bilan sur les micronutriments |

### Critères d'acceptation

- Étant donné une version d'objectif en vigueur depuis le 02/11, quand je change la cible d'énergie le
  15/11, alors une nouvelle version commence le 15/11 et le bilan de la semaine du 02/11 reste calculé
  avec l'ancienne cible (R-NUT-3).
- Étant donné un dîner de 2 400 kcal mangé par 2 adultes et un enfant de coefficient 0,6, alors l'apport
  par équivalent adulte est de 923 kcal (R-NUT-21).
- Étant donné des lasagnes de 6 portions cuisinées pour 3,6 équivalents adulte, dont il reste
  2,4 portions déclarées, alors le repas compte 3,6 portions, et le déjeuner du lendemain qui mange les
  restes compte les 2,4 portions (R-NUT-20, ADR-0012).
- Étant donné une semaine avec un saumon (poisson gras) et un cabillaud (poisson maigre) d'au moins une
  portion par équivalent adulte chacun, alors « poisson ≥ 2 fois » et « poisson gras ≥ 1 fois » sont
  atteints (R-NUT-13, R-NUT-14).
- Étant donné une journée dont le déjeuner est pris à la cantine (part 35 %), alors la cible d'énergie
  du jour est de 1 300 kcal au lieu de 2 000 (R-NUT-31).
- Étant donné un repas dont un ingrédient n'a pas de valeur de fibres, alors le critère « fibres » du
  jour est `INCOMPLETE`, avec la valeur partielle et l'ingrédient en cause (R-NUT-24).
- Étant donné un plat de lentilles cuisiné le lundi et ses restes mangés le mardi, alors « même
  recette » ne compte qu'une occurrence (R-NUT-15).
