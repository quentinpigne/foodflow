# Foodflow — Dossier de conception

Application personnelle de gestion alimentaire du foyer : stock et péremption, recettes,
planification de repas équilibrés, liste de courses, coût et lien avec les budgets de Cashflow.

> **Statut : conception en cours (phase 0).** Aucune ligne de code n'est écrite.
> Ce qui reste à trancher : [questions-ouvertes.md](questions-ouvertes.md).

## Par où commencer

| Je veux… | Lire |
|---|---|
| Comprendre le produit et ses limites | [vision.md](vision.md) |
| Voir ce qui reste à trancher | [questions-ouvertes.md](questions-ouvertes.md) |
| Comprendre pourquoi un choix a été fait | [adr/](adr/) |

À venir : glossaire, modèle de données, architecture, écrans, roadmap, specs par domaine.

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
