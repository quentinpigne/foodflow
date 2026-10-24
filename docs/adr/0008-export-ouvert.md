# ADR-0008 — Export complet dans un format ouvert

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : architecture, ADR-0009

## Contexte

Les données saisies (recettes, préférences, historique de stock et de prix) représentent des années
d'effort. Elles ne doivent pas être prisonnières de l'application.

## Décision

Comme Cashflow (ADR-0008 de Cashflow) : un **export JSON complet et versionné**, réimportable, de toutes
les données saisies (hors données de référence Ciqual, réimportables, et hors cache Open Food Facts), et
des **sauvegardes automatiques** de la base.
Les recettes s'exportent aussi une à une au format schema.org/Recipe (JSON-LD), le même format que
l'import par URL.

## Conséquences

- Le format d'export porte un numéro de version et se documente dans `docs/export.md`.
- Toute nouvelle entité doit être ajoutée à l'export, et l'export testé par un aller-retour.
