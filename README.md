# Foodflow

Application personnelle de gestion alimentaire du foyer : stock et péremption, recettes, planification
de repas équilibrés (à la main, générée, ou proposée par une IA), batch cooking, liste de courses
utilisable en magasin sans réseau, anti-gaspillage, et lien avec les budgets de
[Cashflow](../cashflow).

Mono-utilisateur, auto-hébergée sur le réseau local : les données ne quittent pas la machine sans
action volontaire.

## Statut

Conception en cours (phase 0) : aucune ligne de code n'est écrite. Voir la [roadmap](docs/roadmap.md).

## Stack

Celle de Cashflow : backend Rust (axum, sqlx + SQLite) en deux crates, `domain` (calculs purs) et
`server` ; frontend Angular et Angular Material dans `web/`, installable en PWA ; un seul binaire qui
sert l'API et l'interface en HTTPS, dans une image Docker. Détails : [architecture](docs/architecture.md)
et [ADR-0009](docs/adr/0009-stack-et-deploiement.md).

## Documentation

Toute la conception est dans [`docs/`](docs/README.md). Les consignes pour Claude Code sont dans
[CLAUDE.md](CLAUDE.md).
