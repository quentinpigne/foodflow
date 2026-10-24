# ADR-0011 — Temps disponible et préparation à l'avance (batch cooking)

- **Statut** : Proposé
- **Date** : 2026-10-25
- **Concerne** : `REC`, `PLA`, `STK`, ADR-0002, ADR-0004, ADR-0005

## Contexte

Un menu équilibré ne sert à rien s'il est impossible à cuisiner : le temps disponible varie selon le jour
et le repas (20 minutes un mardi soir, une heure le samedi midi). Pour tenir la semaine, on peut aussi
cuisiner **à l'avance** pendant une longue session, souvent le dimanche : soit des recettes entières
(un chili pour mardi et jeudi), soit des **éléments** de recettes (la sauce, la pâte, les légumes
taillés), qu'il ne reste qu'à finir le jour même.

Il faut donc savoir, pour chaque recette, combien de temps elle demande, ce qu'on peut en préparer à
l'avance et combien de temps cela se conserve ; et, pour chaque repas, combien de temps on a.

## Options envisagées

### Temps d'une recette
1. **Un temps total** — ❌ un bœuf bourguignon (20 min de travail, 3 h de cuisson) passerait pour
   impossible un soir où l'on est présent mais occupé.
2. **Temps actif et temps passif** — ✅ distingue le travail (on est mobilisé) de l'attente (cuisson,
   repos, marinade) / ❌ deux valeurs à saisir.

### Granularité de la préparation à l'avance
1. **Étape par étape** (chaque étape marquée « faisable à l'avance ») — ✅ fin / ❌ lourd à saisir, et
   le résultat d'une étape n'est pas un objet qu'on range au frigo.
2. **Recette entière et sous-recettes** — ✅ une sous-recette (sauce tomate maison, pâte brisée) est une
   recette à part entière, avec ses temps et sa conservation ; la préparer produit un lot rangé et
   consommé comme les autres / ❌ il faut découper les recettes qu'on veut préparer en plusieurs fois.

## Décision

Temps actif et temps passif ; préparation à l'avance par recette entière et par sous-recettes.

### Recettes (spec `REC`)
- Une recette a un **temps actif** et un **temps passif** ; ses sous-recettes ont les leurs.
- Un ingrédient de recette peut être **une autre recette** (sous-recette), en quantité ou en portions.
- Une recette indique si elle **se prépare à l'avance** : conservation au frigo (jours), congelable ou
  non, et **temps de finition** le jour du repas (réchauffer, cuire les pâtes, assembler).

### Temps disponible (spec `PLA`)
- La **semaine type** définit, pour chaque jour et chaque créneau de repas, un **temps actif disponible**
  et une **heure limite** (le repas doit être prêt à 19 h 30). Un jour précis peut être surchargé.
- Elle définit aussi zéro, une ou plusieurs **sessions de préparation** (par défaut : dimanche
  après-midi, 3 h), avec leur temps disponible ; une session peut être ajoutée ou supprimée sur une
  semaine donnée.

### Préparation à l'avance
- Un repas planifié est cuisiné **sur place** (temps actif de la recette et de ses sous-recettes), ou
  **préparé à l'avance**, en tout ou en partie, dans une session de préparation : il ne compte alors
  le jour même que le temps de finition.
- Le **plan de la session** liste ce qu'on y prépare, pour quels repas, et en quelles quantités (mises à
  l'échelle des convives de chaque repas, ADR-0004).
- Valider une session (ADR-0005) sort les ingrédients du stock et crée **un lot de préparation** par
  préparation, avec sa date limite (date de la session + conservation, ou durée au congélateur s'il est
  rangé au congélateur). Le repas consomme ensuite ce lot comme tout autre lot (ADR-0002) : une même
  préparation peut servir plusieurs repas.
- Une préparation n'est proposée pour un repas que si **sa date limite couvre la date du repas** ; au-delà
  de la conservation au frigo, il faut la congeler (et prévoir la décongélation la veille).

### Planification
- Les générations, déterministe ou par IA, respectent : temps actif sur place (ou de finition) ≤ temps
  disponible du créneau ; heure limite respectée compte tenu du temps passif ; Σ des temps actifs d'une
  session ≤ son temps disponible ; conservation des préparations.
- La saisie manuelle n'est pas bloquée : un dépassement est **signalé**, pas interdit.

## Conséquences

- Le lot de « restes » (ADR-0002) devient un cas particulier du **lot de préparation** : une recette
  cuisinée dont il reste des portions.
- La durée d'une session est **estimée** par la somme des temps actifs ; le parallélisme réel (un plat au
  four pendant qu'on en prépare un autre) n'est pas modélisé en V1 : l'estimation est prudente.
- La liste de courses d'une semaine doit être faite **avant** la session de préparation : les besoins
  de la session sont datés du jour de la session, pas du jour des repas.
- Les recettes importées n'ont souvent qu'un temps de préparation et un temps de cuisson : ils sont
  repris comme temps actif et temps passif, à revoir lors de la revue de l'import.
