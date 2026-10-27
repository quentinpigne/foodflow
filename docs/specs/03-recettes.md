# Spec 03 — Recettes

Statut : **Validée** (2026-10-25)
Modèle : [Recipe, RecipeIngredient, RecipeStep, RecipeDraft, IngredientMapping](../modele-de-donnees.md#recettes--spec-03)
Décisions : ADR-0003 (aliments et familles), ADR-0004 (convives), ADR-0005 (revue des imports),
ADR-0007 (services tiers), ADR-0008 (export), ADR-0011 (temps et préparation à l'avance),
ADR-0015 (phases et ordonnancement)

## Concepts

```
Lasagnes à la bolognaise                          PLAT · 6 portions · actif 40 min · passif 50 min
│                                                  se prépare à l'avance : 3 j au frigo, congelable 3 mois
│                                                  finition : 10 min (réchauffer au four)
├─ Pour la sauce
│   └─ 1 recette   Sauce bolognaise  (sous-recette, 900 g)    actif 20 min · passif 1 h 30
├─ Pour la béchamel
│   ├─ 50 g        Beurre doux                   (aliment)
│   ├─ 50 g        Farine de blé                 (aliment)
│   └─ 50 cL       Lait demi-écrémé              (aliment)
├─ 12 feuilles     Pâtes à lasagnes              (aliment, unité propre « feuille » = 20 g)
├─ 100 g           Fromage râpé                  (famille : au choix)
└─ sel, poivre     (à volonté, sans quantité)

Par portion (adulte) : 640 kcal · P 32 g · G 52 g · L 33 g · coût ≈ 2,10 €
                       ⚠ fibres incomplètes : 1 ingrédient sans valeur
```

- **Recette** (`Recipe`) : ingrédients, étapes, rendement, temps, préparation à l'avance, catégorie
  (plat, accompagnement, entrée, dessert, petit-déjeuner, goûter, base) et étiquettes libres.
- **Rendement** (`yield`) : en **portions adulte** (un plat pour 6) ou en **quantité** (une sauce de
  900 g, une pâte de 400 g). C'est la base de la mise à l'échelle.
- **Ingrédient** (`RecipeIngredient`) : un **aliment**, une **famille** (« au choix ») ou une
  **sous-recette** (ADR-0011), avec une quantité, ou **à volonté** (sel, poivre).
- **Étape** (`RecipeStep`) : texte, avec un minuteur facultatif, regroupée en sections facultatives.

## Règles

### Recette

- **R-REC-1** — Une recette a un nom (pas forcément unique : deux « Quiche lorraine » peuvent
  coexister), une catégorie (`RecipeKind`), au moins un ingrédient et un rendement.
- **R-REC-2** — Une recette **archivée** n'est plus proposée par la planification ni par la recherche
  par défaut ; elle reste visible dans l'historique des repas.
- **R-REC-3** — Modifier une recette ne change pas les repas déjà cuisinés : leur consommation réelle a
  été enregistrée à leur validation (spec 05, ADR-0005). Elle change les repas planifiés non encore
  cuisinés.
- **R-REC-4** — Une recette a des **notes personnelles** (« mettre moins de sel »), distinctes de sa
  description d'origine.

### Rendement et mise à l'échelle

- **R-REC-10** — Le rendement en portions s'entend en **portions adulte** (ADR-0004). Une recette importée
  « pour 4 personnes » donne 4 portions adulte, ajustables à la revue.
- **R-REC-11** — Le rendement en quantité est exprimé dans une mesure (`MASS`, `VOLUME`, `COUNT`). Il est
  réservé aux recettes utilisées comme sous-recettes (catégorie `BASE`) ou mangées au poids.
- **R-REC-12** — **Mise à l'échelle** : la quantité de chaque ingrédient pour un repas = quantité de la
  recette × (équivalents adulte du repas ÷ portions de la recette). Pour une sous-recette, la quantité
  demandée ÷ le rendement de la sous-recette donne son facteur, appliqué à ses propres ingrédients.
- **R-REC-13** — Un ingrédient marqué **fixe** (une feuille de laurier, une pincée de sel) n'est pas mis
  à l'échelle.
- **R-REC-14** — La mise à l'échelle se calcule en décimal exact (R-ALI-33). À l'affichage, une quantité
  en `COUNT` est arrondie à la demi-pièce la plus proche (« ≈ 2 ½ œufs »), sans modifier la valeur
  calculée.
- **R-REC-15** — Les temps ne sont **pas** mis à l'échelle : cuisiner une recette en double prend le même
  temps actif en V1 (ADR-0011).

### Ingrédients

- **R-REC-20** — Un ingrédient vise exactement **un** aliment, **une** famille ou **une** sous-recette.
- **R-REC-21** — La quantité d'un ingrédient est saisie dans n'importe quelle unité (R-ALI-30) ; elle est
  convertie dans la mesure de stock de l'aliment (ou de la famille), ou dans l'unité de rendement de la
  sous-recette (portions ou quantité). La saisie d'origine est conservée pour l'affichage. Une quantité
  non convertible (R-ALI-31) est permise, mais signalée : elle rend incomplets les totaux de la recette.
- **R-REC-22** — Un ingrédient **à volonté** n'a pas de quantité : il ne compte ni dans les valeurs
  nutritionnelles ni dans le coût, et il demande seulement qu'on en ait (un lot non épuisé) pour la liste
  de courses.
- **R-REC-23** — Un ingrédient peut être **facultatif** : il est affiché, mais ne bloque ni la
  planification ni la validation d'un repas, et n'entre dans la liste de courses que si on le demande.
- **R-REC-24** — Une famille d'aliments peut désigner un **aliment de référence** (R-ALI-4) : il fournit
  les valeurs nutritionnelles et le prix d'un ingrédient qui vise la famille, marqués estimés. Sans
  aliment de référence, ces valeurs sont inconnues.

### Sous-recettes

- **R-REC-30** — Les sous-recettes ne forment **pas de cycle** : une recette ne peut pas, directement ou
  non, être son propre ingrédient.
- **R-REC-31** — Une sous-recette est une recette comme une autre : elle a ses temps, sa conservation et
  sa préparation à l'avance, et peut être planifiée seule dans une session de préparation (ADR-0011).
- **R-REC-32** — Pour la liste de courses et le stock, une sous-recette se **développe** en ses propres
  ingrédients, sauf si un lot de préparation de cette sous-recette couvre le besoin (spec 02).

### Temps et préparation à l'avance (ADR-0011)

- **R-REC-40** — Une recette a un **temps actif** et un **temps passif** (minutes, ≥ 0), hors
  sous-recettes. Elle peut décrire ses **phases** (ADR-0015) : suite ordonnée de phases actives ou
  passives, avec leur durée et les équipements qu'elles occupent ; ses temps actif et passif en sont
  alors déduits. Sans phases, elle a une phase active puis une phase passive, sans équipement.
- **R-REC-41** — **Temps total d'une recette cuisinée sur place** = durée du **planning** de la recette
  et de ses sous-recettes (ADR-0015) : une sous-recette se termine avant que la recette commence, et
  les phases passives se recouvrent quand les cuisiniers et les équipements le permettent. Le temps
  actif total reste la somme des temps actifs.
- **R-REC-42** — Une recette **se prépare à l'avance** si elle déclare sa **conservation au frigo**
  (jours). Elle peut aussi déclarer qu'elle est **congelable**, avec une durée au congélateur, et un
  **temps de finition** (minutes, actif) pour le jour du repas.
- **R-REC-43** — Une recette sans conservation déclarée ne peut pas être préparée à l'avance ; ses restes
  suivent la durée générale (R-STK-34).
- **R-REC-44** — Une recette peut déclarer un **aliment de reste** (poulet rôti → « Poulet, viande
  rôtie ») : ses restes peuvent alors être déclarés comme un lot de cet aliment, en quantité, et servir
  d'ingrédient à d'autres recettes (ADR-0012).

### Valeurs calculées

- **R-REC-50** — **Valeurs nutritionnelles** d'une recette = Σ, pour chaque ingrédient avec quantité,
  quantité (en g ou en mL selon la base nutritionnelle de l'aliment, R-ALI-23) × valeur pour 100 ÷ 100 ;
  une sous-recette contribue au prorata de la quantité utilisée. Les pertes et gains à la cuisson ne
  sont pas modélisés (R-ALI-12).
- **R-REC-51** — Elles sont affichées **par portion adulte** (rendement en portions), ou **pour 100 g ou
  100 mL** de préparation (rendement en quantité).
- **R-REC-52** — Un total est **incomplet** si un ingrédient avec quantité a une valeur inconnue, ou une
  quantité non convertible (R-ALI-20) ; l'affichage dit combien d'ingrédients manquent et lesquels.
- **R-REC-53** — **Coût** d'une recette = Σ quantité × prix de référence de l'aliment (spec 06) ; même
  règle d'incomplétude. Il est affiché par portion et pour la recette.
- **R-REC-54** — Les valeurs calculées d'une recette sont **toujours recalculées** (jamais stockées comme
  vérité) : elles suivent les modifications des aliments et des prix.

### Import depuis une URL

- **R-REC-60** — L'import par URL (ADR-0007) télécharge la page côté serveur et lit les données
  **schema.org/Recipe** au format JSON-LD. Sans elles, l'import échoue avec un message clair ; aucune
  extraction approximative du texte de la page n'est tentée.
- **R-REC-61** — L'import produit un **brouillon** (`RecipeDraft`), conservé jusqu'à sa validation ou son
  abandon (ADR-0005). Rien n'est créé avant la validation.
- **R-REC-62** — Champs repris : nom, description, rendement (`recipeYield`), temps (`prepTime` → actif,
  `cookTime` → passif ; à défaut, `totalTime` → actif), ingrédients (`recipeIngredient`), étapes et
  sections (`recipeInstructions`), catégorie et étiquettes (`recipeCategory`, `recipeCuisine`,
  `keywords`), image, URL d'origine. Les valeurs nutritionnelles de la page sont ignorées.
- **R-REC-63** — Chaque ligne d'ingrédient est **analysée** (fonction pure du domaine) en quantité
  (entier, décimal à virgule ou à point, fraction, `½`, intervalle « 2 à 3 » → borne basse, signalée ;
  « un », « une » valent 1, « quelques » ne donne pas de quantité), unité (unités générales et propres,
  avec leurs abréviations : « cs », « c. à s. », « cuil. à soupe » ; un contenant — bouquet, boîte,
  rouleau, pavé… — seulement devant « de », pour que « 1 bouquet garni » reste un nom ; un adjectif devant
  l'unité est ignoré : « 1 petite boîte de maïs »), nom de l'ingrédient et précision (« émincé », « bien
  mûres »). Une alternative (« 4 citrons jaunes ou 8 verts ») ou un usage (« chocolat pour la pâte »)
  passe en précision. « Sel, poivre » donne deux ingrédients à volonté ; « 1 œuf + 1 blanc d'œuf », deux
  ingrédients. Pour un site qui omet le « de » après un contenant et écrit « unité » pour une pièce
  (journaldesfemmes.fr : « 1 sachet levure », « 3 unité pomme »), la ligne est d'abord réécrite dans la forme
  courante ; la ligne d'origine est gardée telle quelle (R-REC-65). Le nom de la recette perd l'accroche
  ajoutée par certains sites (« … : la meilleure recette »), et les mots-clés leur préfixe « recette ».
- **R-REC-64** — Le nom analysé est **rattaché** à une cible (aliment, famille, sous-recette), dans cet
  ordre : rattachement mémorisé (R-REC-66), **dictionnaire de départ** (`seed/ingredient_dictionary.json` :
  quelque 700 noms courants des recettes → aliment Ciqual, « farine » → farine de blé T55), nom ou alias
  exact, recherche approchée. La recherche approchée exige que le **nom principal** de l'ingrédient soit
  celui du candidat (« avocat » n'est pas « Huile d'avocat », « citron » pas « Tarte au citron ») ; les
  qualificatifs de la recette (« fondu », « frais », « rouge ») départagent sans être exigés ; à égalité, la
  forme brute l'emporte sur une forme cuite ou transformée (purée, conserve, jus). Chaque proposition a une
  **confiance** (`HIGH`, `MEDIUM`, `LOW`, `NONE`) ; la revue met en tête les lignes de faible confiance.
  Critère mesuré : au moins 90 % des lignes typiques de `fixtures/recipes/lignes-ingredients.json` sont
  bien rattachées, et au moins 70 % sans le dictionnaire.
- **R-REC-65** — La validation exige que chaque ingrédient ait une cible ; une quantité non convertible
  reste permise (R-REC-21). La ligne d'origine est conservée sur l'ingrédient.
- **R-REC-66** — Chaque rattachement validé est **mémorisé** (nom normalisé → cible) et proposé en
  priorité aux imports suivants, avant le dictionnaire de départ. Un aliment nouveau peut être créé depuis
  la revue (« Créer l'aliment ») : son nom reprend celui de l'ingrédient, et le rattachement est mémorisé à
  la validation.
- **R-REC-67** — Importer une URL déjà importée prévient et propose d'ouvrir la recette existante ;
  on peut tout de même créer une autre recette.
- **R-REC-68** — L'image est téléchargée au moment de l'import et conservée localement ; la recette ne
  dépend plus du site d'origine.
- **R-REC-69** — L'**import par lot** accepte une liste d'adresses (une par ligne, ou un texte qui en
  contient). Les adresses en double, et celles qui ont déjà un brouillon en attente, sont écartées ; les
  autres sont importées **une par une**, avec une pause entre deux pages, jamais en parallèle. Chacune
  donne un brouillon ou une erreur affichée à côté de son adresse ; un échec n'arrête pas le lot. La
  revue enchaîne ensuite les brouillons : valider ou abandonner l'un ouvre le suivant.
- **R-REC-71** — Foodflow ne parcourt jamais un site de lui-même (ni liens, ni plan du site) : il ne
  télécharge que les pages que l'utilisateur lui a désignées.
- **R-REC-72** — **Envoi depuis le navigateur** : un favori (« bookmarklet ») lit les données JSON-LD de la
  page affichée et les ouvre dans Foodflow avec l'adresse de la page ; le serveur ne retélécharge pas la
  page, et l'adresse devient l'URL d'origine. Sur le téléphone, Foodflow installé (PWA) reçoit les
  adresses partagées depuis une autre application et en lance l'import. Les deux mènent à la revue
  habituelle (R-REC-61).
- **R-REC-73** — **Photo libre** : un brouillon créé sans image (réponse de l'IA, page ou fichier sans image)
  reçoit une photo de Pixabay, cherchée d'après le nom de la recette (puis, sans résultat, d'après ses mots
  principaux), si ce service est activé dans les réglages avec sa clé (ADR-0007). Seul le nom part ; la
  photo est copiée dans le volume (R-REC-68) et se remplace dans la revue comme toute image. Sans réglage,
  sans résultat ou en cas d'échec, la recette reste sans photo, sans erreur.

### Création par l'IA (ADR-0014)

- **R-REC-80** — Avec l'IA activée, on peut demander **une recette** à partir d'une consigne libre, du stock
  à écouler (lots `SOON`, restes, lots jugés encore bons), d'une recette existante (variante ; l'originale
  ne change pas), ou au **choix de l'IA**, d'après le stock et les préférences du foyer.
- **R-REC-81** — Les données envoyées sont **affichées avant l'envoi** et limitées au nécessaire : membres
  désignés par une lettre, avec leurs préférences, jamais leur nom (ADR-0007).
- **R-REC-82** — La réponse est une recette au format imposé (nom, catégorie, rendement, temps, lignes
  d'ingrédients, étapes) ; elle devient un **brouillon** (R-REC-61) relu comme un import (R-REC-63 à
  R-REC-66). Une réponse invalide est refusée avec un message, jamais corrigée en silence.
- **R-REC-83** — Chaque appel compte dans le **plafond mensuel** des réglages ; au-delà, la demande est
  refusée jusqu'au mois suivant.

### Export

- **R-REC-70** — Une recette s'exporte seule au format schema.org/Recipe JSON-LD (ADR-0008), avec les
  quantités dans l'unité saisie. Réimporter ce fichier redonne la même recette, rattachements compris
  lorsqu'ils sont mémorisés.

## Features

| ID | Priorité | Feature |
|---|---|---|
| REC-01 | MVP | Saisie d'une recette : ingrédients (aliment, famille, à volonté, facultatif, fixe), étapes, sections, rendement, catégorie, étiquettes |
| REC-02 | MVP | Fiche recette : quantités mises à l'échelle d'un nombre d'équivalents adulte choisi |
| REC-03 | MVP | Valeurs nutritionnelles par portion ou pour 100, avec incomplétude détaillée |
| REC-04 | V1 | Coût par portion et par recette (spec 06) |
| REC-05 | MVP | Temps actif et passif ; préparation à l'avance : conservation, congélation, finition |
| REC-06 | V1 | Sous-recettes, développement des ingrédients, contrôle des cycles |
| REC-07 | V1 | Import depuis une URL (schema.org JSON-LD) : brouillon, analyse des lignes, revue, rattachements mémorisés |
| REC-08 | V1 | Mode cuisine sur le téléphone : étape par étape, quantités mises à l'échelle, minuteurs, écran maintenu allumé |
| REC-09 | V1 | Recherche et filtres : catégorie, étiquette, temps actif maximum, préparable à l'avance, ingrédient présent, ce qui est en stock |
| REC-10 | V1 | Photo de la recette (import, téléversement, ou photo libre de Pixabay, R-REC-73) |
| REC-11 | V1 | Export d'une recette en schema.org JSON-LD, réimport |
| REC-12 | V1 | Archivage, notes personnelles, duplication d'une recette pour en faire une variante |
| REC-16 | V1 | Aliment de reste ; recettes qui utilisent un reste (par sous-recette ou par aliment de reste) |
| REC-13 | V2 | Substitutions proposées pour un ingrédient (une famille, ou un aliment de remplacement déclaré) |
| REC-14 | V2 | Import d'une page sans JSON-LD par extraction IA (service tiers optionnel) |
| REC-15 | V1 | Phases d'une recette et équipements qu'elles occupent (ADR-0015) |
| REC-17 | V1 | Import par lot d'adresses, revue enchaînée des brouillons |
| REC-18 | V1 | Envoi depuis le navigateur : favori sur l'ordinateur, partage vers la PWA sur le téléphone |
| REC-19 | V1 | Création d'une recette par l'IA : consigne, stock à écouler, variante, choix de l'IA (ADR-0014) |

### Critères d'acceptation

- Étant donné une recette de 6 portions demandant 3 œufs, quand je l'affiche pour 2 adultes et un enfant
  de coefficient 0,6 (2,6 équivalents adulte), alors elle demande 1,3 œuf, affiché « ≈ 1 ½ œuf »
  (R-REC-12, R-REC-14).
- Étant donné une recette avec une feuille de laurier marquée fixe, quand je la double, alors elle
  demande toujours une feuille (R-REC-13).
- Étant donné des lasagnes qui utilisent 900 g de sauce bolognaise, et une sauce bolognaise dont le
  rendement est de 1 800 g, alors les lasagnes comptent la moitié des ingrédients de la sauce, et leur
  temps actif total ajoute celui de la sauce (R-REC-12, R-REC-41).
- Étant donné une sauce (20 min actives puis 1 h 30 passives) et un gratin (15 min actives puis 45 min de
  four), sans sous-recette entre eux, cuisinés par un cuisinier, alors le planning taille le gratin
  pendant que la sauce mijote et dure 1 h 50, et non 2 h 50 (R-REC-41, ADR-0015).
- Étant donné une recette A qui utilise B, quand j'ajoute A comme ingrédient de B, alors c'est refusé
  (R-REC-30).
- Étant donné une recette dont un ingrédient est « 2 verres de farine » sans densité connue, alors ses
  valeurs nutritionnelles sont marquées incomplètes et l'ingrédient en cause est nommé (R-REC-21,
  R-REC-52).
- Étant donné la ligne importée « 1/2 c. à soupe d'huile d'olive vierge extra », alors l'analyse donne
  0,5 cuillère à soupe, ingrédient « huile d'olive », précision « vierge extra » (R-REC-63).
- Étant donné que j'ai rattaché « gousses d'ail » à l'aliment « Ail » lors d'un import, alors l'import
  suivant contenant « 3 gousses d'ail » le propose avec une confiance `HIGH` (R-REC-66).
- Étant donné une page sans données schema.org/Recipe, alors l'import échoue avec un message qui le dit,
  et aucun brouillon n'est créé (R-REC-60).
