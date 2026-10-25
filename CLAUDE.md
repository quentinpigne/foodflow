# CLAUDE.md

Consignes pour Claude Code sur le projet Foodflow (gestion alimentaire du foyer).

## Phase actuelle : implémentation de la V1

La conception est terminée et validée (phase 0, 2026-10-25). On implémente la V1 phase par phase
(`docs/roadmap.md`). Le propriétaire corrigera la conception à l'usage : un écart constaté se reporte
dans la spec ou l'ADR concerné, dans le même commit que le code.
Questions encore ouvertes, **à ne jamais trancher seul** (proposer, puis demander) :
`docs/questions-ouvertes.md`.

Foodflow reprend la stack, les conventions et l'organisation de **Cashflow** (`../cashflow`), qui sert
de référence : en cas de doute sur une convention de code, de test, de CI ou de déploiement, faire comme
Cashflow.

## Carte du dépôt

- `docs/README.md` : point d'entrée du dossier de conception.
- `docs/vision.md` : objectifs, non-objectifs, principes produit.
- `docs/glossaire.md` : vocabulaire métier FR ↔ identifiants EN. **Source de vérité des noms.**
- `docs/modele-de-donnees.md` : entités, champs, contraintes. **Source de vérité du schéma.**
- `docs/architecture.md` : architecture et projection sur la stack.
- `docs/specs/NN-*.md` : règles (`R-XXX-n`) et features (`XXX-nn`) par domaine.
- `docs/adr/` : décisions d'architecture.
- Pas de document d'écrans : les écrans sont implémentés directement en s'inspirant de Cashflow
  (`../cashflow/web`), puis corrigés visuellement par le propriétaire.

## Règles de rédaction

Celles de Cashflow :

- Documentation en français, identifiants de code en anglais (cf. glossaire).
- Un concept = un nom. Avant d'introduire un terme, vérifier le glossaire ; s'il manque, l'y ajouter.
- Toute modification du modèle se fait **à la fois** dans `docs/modele-de-donnees.md` et dans la spec du
  domaine concerné.
- Nouvelle décision structurante → nouvel ADR (copier `docs/adr/template.md`, numéro suivant, index).
- Tant que l'application n'a pas de données réelles en production, un ADR se modifie directement ;
  ensuite, un nouvel ADR remplace l'ancien.
- Identifiants `R-*` et `<DOMAINE>-nn` stables : ne jamais renuméroter, marquer `(supprimée)`.

## Invariants non négociables

1. **Jamais de flottant** pour une quantité, une valeur nutritionnelle, un prix ou un coefficient :
   entiers dans l'unité de stockage, décimaux exacts dans les calculs — ADR-0001.
2. **Le stock se calcule** à partir du journal de mouvements ; un mouvement validé ne se modifie pas,
   il s'annule par un mouvement inverse — ADR-0002.
3. **Rien ne bouge sans validation explicite** : courses, repas, sessions de préparation, imports,
   propositions de menus — ADR-0005.
4. **Une valeur inconnue n'est jamais zéro** : un total qui l'inclut est signalé incomplet, avec ce qui
   manque — R-ALI-20.
5. **Génération et ordonnancement sont déterministes** : mêmes entrées (et même graine) ⇒ même
   résultat — ADR-0013, ADR-0015.
6. **Services tiers à la demande et désactivables** ; les données envoyées à l'IA sont affichées avant
   l'envoi, sans nom de membre — ADR-0007, ADR-0014.
7. **Foodflow n'écrit jamais dans Cashflow** et n'en stocke que des identifiants — ADR-0010.
8. **Aucun secret ni aucune donnée personnelle dans le dépôt** (base, images, jetons, clés, certificats).

## Stack (ADR-0009)

Celle de Cashflow : Rust (`crates/{domain,server}`, axum, sqlx + SQLite sans ORM, rust_decimal, chrono,
uuid v7) et Angular (standalone, signals, sans zone.js, Angular Material, Reactive Forms typés, ECharts),
binaire unique, Docker sur ubuntu01. En plus : HTTPS servi par axum dès la V1, PWA (service worker, mode
magasin hors connexion, Web Push).

## Règles de code

Celles de Cashflow (`../cashflow/CLAUDE.md`, « Règles de code »), notamment :

- `domain` : aucune I/O, aucun flottant, aucune dépendance à tokio, sqlx, axum ou reqwest ; la date du
  jour est un paramètre.
- `server` : les routes n'exécutent pas de requête, les services ne calculent pas, toute requête passe
  par `db`. Les traits sont réservés aux clients de services extérieurs (Open Food Facts, pages web, IA,
  Cashflow, Web Push), pour les remplacer en test : **aucun test n'appelle le réseau**.
- Pas de `unwrap()`/`expect()` hors tests et démarrage ; erreurs métier en enums `thiserror` qui portent
  le code de la règle `R-*`. Pas d'`unsafe`. Édition 2024, pas de `mod.rs`.
- Migrations immuables une fois publiées ; requêtes `sqlx::query!` vérifiées à la compilation ; tables
  `STRICT`.
- Côté TS : client d'API généré, jamais écrit à la main ; aucune arithmétique sur des quantités ou des
  montants hors de `quantity.ts` et `money.ts`.
- Chaque règle `R-*` a au moins un test qui la cite ; les exemples chiffrés des specs sont repris tels
  quels comme tests.

## Commits

Un commit par étape cohérente, directement sur `main`, message en français, **sans** ligne
`Co-Authored-By` ni autre attribution. Les messages référencent les features concernées (ex. `STK-03`).
