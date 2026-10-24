# ADR-0013 — Génération déterministe des menus

- **Statut** : Proposé
- **Date** : 2026-10-25
- **Concerne** : `PLA`, `NUT`, `STK`, `REC`, ADR-0004, ADR-0005, ADR-0011, ADR-0012

## Contexte

Remplir une semaine à la main est long. La génération automatique doit tenir ensemble beaucoup de
choses : les objectifs nutritionnels (spec 04), les goûts des convives (ADR-0004), le temps disponible
et les sessions de préparation (ADR-0011), les restes et ce qui va périmer (ADR-0012, spec 02), la
variété, et le coût. La vision exige qu'**une même demande, avec les mêmes données, donne toujours le
même menu**, et que le résultat soit compréhensible.

## Options envisagées

1. **Programmation linéaire en nombres entiers** (solveur externe) — ✅ optimum prouvé / ❌ dépendance
   lourde, modélisation difficile des critères de fréquence et de variété, résultats peu explicables,
   temps de calcul imprévisible.
2. **Tirage aléatoire filtré** — ✅ simple / ❌ non reproductible, qualité médiocre.
3. **Heuristique déterministe : contraintes dures, score pondéré, construction gloutonne puis recherche
   locale, départage par graine** — ✅ pure, testable, reproductible, explicable, assez rapide pour
   quelques centaines de recettes / ❌ pas d'optimum garanti.

## Décision

Option 3, implémentée comme une **fonction pure du domaine** (ADR-0009 : aucune I/O, aucune horloge).

### Entrées
Période et créneaux à remplir (vides et non verrouillés), repas déjà planifiés (verrouillés), semaine type
et convives de chaque repas, composition type des créneaux, version d'objectif en vigueur, préférences,
recettes non archivées, stock (lots, dates limites, restes), sessions de préparation de la période,
prix de référence, réglages de génération (poids), et une **graine**.

### Contraintes dures
Un repas proposé ne peut pas :
- contenir un aliment, une famille ou une recette **refusés** par un convive présent ;
- dépasser le **temps actif disponible** du créneau (temps sur place, ou de finition s'il est préparé à
  l'avance), ni l'**heure limite** compte tenu du temps passif ;
- utiliser une préparation ou un reste **après sa date limite**, ou dépasser le temps d'une session de
  préparation ;
- violer un critère d'objectif d'importance `REQUIRED`, si une solution qui le respecte existe ; sinon
  le meilleur résultat est proposé et la violation signalée.

### Score
Somme pondérée, les poids étant des réglages avec des valeurs par défaut :
- écart aux critères `PREFERRED` (normalisé par critère) ;
- préférences des convives présents (`LOVES` +, `LIKES` +, `DISLIKES` −), pondérées par leurs
  équivalents adulte ;
- usage du stock : restes et lots `SOON` fortement favorisés, puis le stock en général ;
- coût de la période, si un plafond est donné (saisi, ou reste du budget lu dans Cashflow, ADR-0010) ;
- variété ;
- effort : temps actif total, nombre de recettes différentes à cuisiner.

### Algorithme
1. Construction **gloutonne**, créneau par créneau dans l'ordre chronologique : pour chaque rôle de la
   composition type, le meilleur candidat admissible au regard du score courant.
2. **Recherche locale** bornée (remplacer un élément, échanger deux repas, cuisiner pour plus et affecter
   le reste, déplacer une recette vers une session de préparation) tant que le score s'améliore, avec un
   nombre maximal d'itérations.
3. Tout ordre et tout départage passent par une **clé stable** (score, puis identifiant), puis par un
   générateur pseudo-aléatoire initialisé avec la graine. **Mêmes entrées et même graine ⇒ même menu.**
   « Une autre proposition » change la graine.

### Sortie
Une **proposition** (ADR-0005), jamais écrite directement : chaque élément proposé porte ses **raisons**
(« utilise la crème qui périme le 28/10 », « 2 convives aiment »), et la proposition s'accompagne du bilan
nutritionnel prévu et des contraintes non satisfaites. On l'accepte en tout ou en partie.

## Conséquences

- Les tests de la génération sont des tests de domaine purs, avec des jeux de données fixes et des
  propriétés vérifiables (aucune contrainte dure violée, reproductibilité, monotonie du score).
- La qualité dépend des poids : ils sont réglables, et leurs valeurs par défaut seront ajustées à l'usage.
- Le temps de calcul doit rester sous quelques secondes pour une semaine ; un budget d'itérations le
  garantit.
- La génération par IA (ADR-0014) réutilise les contraintes dures et le calcul du score pour contrôler
  et présenter ses propositions.
