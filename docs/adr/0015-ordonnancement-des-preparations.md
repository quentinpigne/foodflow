# ADR-0015 — Ordonnancement des préparations

- **Statut** : Proposé
- **Date** : 2026-10-25
- **Concerne** : `REC`, `PLA`, ADR-0011, ADR-0013

## Contexte

L'ADR-0011 estimait la durée d'une session de préparation par la somme des temps actifs, sans modéliser
le parallélisme. Décision du propriétaire (2026-10-25) : **l'ordre de cuisson d'une session doit être
optimisé dès la V1**. Une session de batch cooking n'a d'intérêt que si l'on profite des temps passifs :
pendant que le chili mijote, on taille les légumes du gratin ; pendant que le gratin est au four, on
prépare la sauce. À l'inverse, deux plats qui veulent le four en même temps doivent attendre leur tour.

Pour ordonnancer, il faut savoir, pour chaque recette : l'enchaînement de ses phases actives et passives,
et les **équipements** que chaque phase occupe (four, feux, robot, autocuiseur).

## Options envisagées

### Description d'une recette
1. **Temps actif et temps passif seulement** — ❌ on ne sait ni dans quel ordre ils viennent, ni ce
   qu'ils occupent.
2. **Phases** : une suite de phases actives ou passives, chacune avec sa durée et ses équipements,
   facultative, avec une valeur par défaut déduite des temps actif et passif — ✅ fin quand on le
   veut, sans saisie obligatoire / ❌ un peu de saisie pour les recettes qu'on prépare souvent.
3. **Étapes de la recette annotées** — ❌ couple l'affichage des étapes et l'ordonnancement, trop lourd
   (Q17).

### Calcul
1. **Solveur d'ordonnancement exact** — ❌ dépendance lourde, temps imprévisible.
2. **Ordonnancement de liste déterministe** (priorité au chemin critique, puis aux longues phases
   passives) — ✅ pur, reproductible, rapide, très bon sur une dizaine de recettes / ❌ pas d'optimum
   garanti.

## Décision

Option 2 pour la description, option 2 pour le calcul.

### Phases et équipements
- Une recette peut décrire ses **phases** (`RecipePhase`) : nature (`ACTIVE` ou `PASSIVE`), durée,
  équipements occupés, dans l'ordre. Une phase passive qui demande une surveillance légère reste passive.
- Sans phases décrites, une recette a par défaut **une phase active** (son temps actif) **puis une phase
  passive** (son temps passif), sans équipement.
- La cuisine déclare ses **équipements** (`Equipment`) et leur capacité : four (1), feux (4),
  robot (1)… Une phase occupe une unité de chaque équipement qu'elle cite pendant toute sa durée.
- Les temps actif et passif d'une recette restent saisis : quand des phases existent, ils en sont
  **déduits** (somme des phases de chaque nature).

### Ordonnancement
- Fonction pure du domaine : à partir d'un ensemble de recettes à cuisiner (avec leurs sous-recettes),
  d'un **nombre de cuisiniers** (1 par défaut) et des équipements, elle produit un **planning** : pour
  chaque phase, son heure de début et de fin.
- Contraintes : les phases d'une recette se suivent dans l'ordre ; une recette commence après la fin de
  ses sous-recettes ; une phase active occupe un cuisinier ; les équipements ne sont jamais
  surchargés.
- Objectif : terminer **au plus tôt** (durée totale minimale). Départage par clé stable : même entrée,
  même planning.
- Elle s'applique :
  - aux **sessions de préparation** : la durée de la session est celle du planning (et non plus la somme
    des temps actifs), comparée au temps disponible ;
  - aux **repas cuisinés sur place**, avec leurs différents éléments (plat, accompagnement) : le
    planning, calé pour finir à l'heure limite, dit si le repas est faisable et à quelle heure commencer.
- Le planning s'affiche en **mode cuisine** : une frise par cuisinier et par équipement, et la prochaine
  action à faire (« 15:20 — enfourner le gratin, 45 min »).

## Conséquences

- L'ADR-0011 est modifié en conséquence : la durée d'une session n'est plus estimée par la somme des
  temps actifs.
- La génération (ADR-0013) utilise l'ordonnancement pour vérifier les contraintes de temps ; pour rester
  rapide, elle peut d'abord filtrer par la somme des temps actifs, puis ne calculer le planning que des
  candidats retenus.
- La qualité du planning dépend des phases saisies : une recette sans phases donne un planning prudent
  (actif puis passif, sans équipement).
- Les températures de four différentes ne sont pas modélisées : un four de capacité 1 n'accueille qu'un
  plat à la fois. Déclarer une capacité de 2 permet deux plats à la même température.
