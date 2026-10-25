# Foodflow

Application personnelle de gestion alimentaire du foyer : stock et péremption, recettes, planification
de repas équilibrés (à la main, générée, ou proposée par une IA), batch cooking, liste de courses
utilisable en magasin sans réseau, anti-gaspillage, et lien avec les budgets de
[Cashflow](../cashflow).

Mono-utilisateur, auto-hébergée sur le réseau local : les données ne quittent pas la machine sans
action volontaire.

## Statut

Conception terminée (2026-10-25) ; l'implémentation de la V1 suit la [roadmap](docs/roadmap.md). La phase 1
(socle, aliments, import Ciqual, stock) est en place, sauf le HTTPS (Q16).

## Stack

Celle de Cashflow : backend Rust (axum, sqlx + SQLite) en deux crates, `domain` (calculs purs) et
`server` ; frontend Angular et Angular Material dans `web/`, installable en PWA ; un seul binaire qui
sert l'API et l'interface en HTTPS, dans une image Docker. Détails : [architecture](docs/architecture.md)
et [ADR-0009](docs/adr/0009-stack-et-deploiement.md).

## Démarrer

- **Développement (Mac)** : `cargo run -p foodflow-server -- demo` crée une base de démonstration fictive
  (mot de passe `demo-foodflow`), puis `cargo run -p foodflow-server -- serve` (port 3100) et, dans `web/`,
  `npm install && npm start`. L'interface est sur `http://localhost:4200`.
- **Table Ciqual** : `cargo run -p foodflow-server -- ciqual-import <dossier des XML Ciqual>`.
- **Serveur** : `deploy/deploy.sh`, cf. [deploy/README.md](deploy/README.md).

## Documentation

Toute la conception est dans [`docs/`](docs/README.md). Les consignes pour Claude Code sont dans
[CLAUDE.md](CLAUDE.md).
