# Foodflow — Dossier de conception

Application personnelle de gestion alimentaire du foyer : stock et péremption, recettes,
planification de repas équilibrés, liste de courses, coût et lien avec les budgets de Cashflow.

> **Statut : conception en cours (phase 0).** Aucune ligne de code n'est écrite.
> Ce qui reste à trancher : [questions-ouvertes.md](questions-ouvertes.md).

## Par où commencer

| Je veux… | Lire |
|---|---|
| Comprendre le produit et ses limites | [vision.md](vision.md) |
| Connaître le vocabulaire métier | [glossaire.md](glossaire.md) |
| Voir le modèle de données | [modele-de-donnees.md](modele-de-donnees.md) |
| Lire les règles et features d'un domaine | [specs/](specs/) : [01 aliments](specs/01-aliments.md), [02 stock](specs/02-stock.md), [03 recettes](specs/03-recettes.md),
  [04 objectifs nutritionnels](specs/04-objectifs-nutritionnels.md),
  [05 planification](specs/05-planification.md), [06 courses](specs/06-courses.md) |
| Voir ce qui reste à trancher | [questions-ouvertes.md](questions-ouvertes.md) |
| Comprendre pourquoi un choix a été fait | [adr/](adr/README.md) |

À venir : specs des autres domaines, architecture, écrans, roadmap.

## Conventions de la documentation

Elles reprennent celles de Cashflow.

- **Langue** : documentation en français ; identifiants (entités, champs, enums) en anglais.
  La correspondance sera dans le glossaire.
- **Identifiants de traçabilité** :
  - Règles métier : `R-<DOMAINE>-<n>` (ex. `R-STK-3`). Chaque règle doit avoir au moins un test.
  - Features : `<DOMAINE>-<nn>` (ex. `COU-04`), avec une priorité `MVP`, `V1` ou `V2`.
  - Codes domaine : `ALI` aliments, `STK` stock, `REC` recettes, `NUT` objectifs nutritionnels,
    `PLA` planification, `COU` courses, `GAS` anti-gaspillage, `CSH` lien avec Cashflow,
    `TDB` tableau de bord.
- **Statut des documents** : chaque spec indique en tête `Brouillon`, `Revue` ou `Validée`.
- **Décisions** : toute décision structurante passe par un ADR (copier [adr/template.md](adr/template.md)).
