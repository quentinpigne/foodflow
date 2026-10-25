# Roadmap

Statut : **Validée** (2026-10-25)

Ordre de réalisation proposé. Chaque phase livre quelque chose d'**utilisable** et repose sur la
précédente. Les identifiants renvoient aux features des specs. Les écrans de chaque phase sont
implémentés directement, en s'inspirant de Cashflow, puis corrigés visuellement par le propriétaire.

## Phase 0 — Conception

- [x] Vision, ADR 0001 à 0015 (2026-10-24 et 2026-10-25)
- [x] Specs 01 à 08 validées (2026-10-25)
- [x] Spec 09, vision, architecture, roadmap, glossaire, modèle de données validés (2026-10-25)
- [x] `CLAUDE.md` (2026-10-25)

## Phase 1 — Socle et stock (MVP)

Objectif : décrire sa cuisine et tenir un stock exact.

- Squelette : workspace Cargo (`domain`, `server`), `web/` (Angular), CI, image Docker, déploiement
  (comme Cashflow)
- HTTPS servi par axum avec rechargement du certificat, renouvellement Let's Encrypt sur ubuntu01
  (ADR-0009 ; mécanisme à confirmer, Q16) ; authentification ; sauvegardes automatiques
- Chaîne OpenAPI → client TypeScript ; types `Amount`, `Measure`, `Money`, `Ratio` et leurs tests ;
  `quantity.ts`
- ALI-01 à ALI-06 — familles, aliments, import Ciqual, unités et conversions, produits, recherche
- STK-01 à STK-05 — emplacements, lots, mouvements, péremption, inventaire
- Réglages (`Setting`), données initiales, jeu de démonstration, export JSON (ADR-0008)
- TDB-01 (version minimale : à consommer en priorité)

**Critère de sortie** : la table Ciqual est importée ; le contenu réel du frigo, du congélateur et du
placard est saisi ; après une semaine de mouvements et un inventaire, les écarts sont expliqués.

**État (2026-10-25)** : tout est implémenté sauf le HTTPS et le renouvellement du certificat, en attente
de Q16. Reste à faire l'essai réel du critère de sortie.

## Phase 2 — Recettes et nutrition (MVP)

Objectif : un carnet de recettes avec des valeurs nutritionnelles honnêtes.

- REC-01, REC-02, REC-03, REC-05 — saisie, mise à l'échelle, valeurs nutritionnelles, temps et
  préparation à l'avance
- REC-06 (sous-recettes), REC-07 (import par URL), REC-08 (mode cuisine), REC-09 à REC-12, REC-16
- NUT-01 à NUT-04 — objectif versionné, groupes et portions, modèle PNNS, bilan

**Critère de sortie** : 20 recettes réelles saisies ou importées, dont la revue d'import prend moins de
2 minutes ; leurs valeurs incomplètes disent ce qui manque.

## Phase 3 — Planification et validation (MVP)

Objectif : planifier la semaine du foyer et tenir le stock par la validation des repas.

- PLA-01 à PLA-05 — membres, semaine type, calendrier, validation des repas, signalements
- PLA-06 à PLA-09 — préférences, composition type, repas habituels, restes (ADR-0012)
- STK-08 — lots de préparation et de restes
- NUT-05 à NUT-07 — critères de groupes, variété, créneaux et hors foyer
- PLA-13 — copier une semaine

**Critère de sortie** : deux semaines réelles planifiées à la main et validées repas par repas ; le stock
affiché correspond au stock réel pour les aliments suivis en quantité.

## Phase 4 — Courses (MVP puis V1)

Objectif : n'acheter que ce qui manque, liste en main en magasin.

- COU-01 à COU-04 — besoins, couverture datée, liste, ajustements, validation vers le stock
- COU-05 à COU-09 — rayons et magasins, mode magasin hors connexion (PWA), scan, prix, stock minimum
- ALI-07, ALI-08 — scan du code-barres, Open Food Facts
- REC-04 — coût des recettes

**Critère de sortie** : quatre sessions de courses réelles faites avec la liste, dont une sans réseau ;
la validation des courses prend moins de 5 minutes.

## Phase 5 — Batch cooking (V1)

- STK-06, STK-07 — ouverture, congélation et décongélation
- REC-15, PLA-15 — phases des recettes, équipements
- PLA-10, PLA-14 — sessions de préparation, ordonnancement, mode cuisine de la session (ADR-0011,
  ADR-0015)

**Critère de sortie** : une session réelle de batch cooking suivie avec le planning, dont la durée réelle
est proche de la durée calculée.

## Phase 6 — Anti-gaspillage (V1)

- GAS-01 à GAS-07 — liste prioritaire, lots sans usage prévu, actions, suggestions, notifications dans
  l'application et push, statistiques
- STK-09, STK-10 — stock minimum, sortie rapide par scan
- TDB-02 à TDB-04 — encadrés de l'accueil, pages de bilan

## Phase 7 — Génération des menus (V1)

- PLA-11 — génération déterministe (ADR-0013)
- PLA-12 — génération par IA (ADR-0014), désactivée par défaut

**Critère de sortie** : une semaine générée acceptée avec moins de 3 modifications ; la même demande
redonne la même proposition.

## Phase 8 — Lien avec Cashflow (V1)

- CSH-00 — **dans Cashflow** : jetons d'accès en lecture seule (ADR dédié dans le dépôt Cashflow)
- CSH-01 à CSH-04 — configuration, budget et projection, plafond de génération, rapprochement

**Critère de sortie** : un mois de courses rapproché de ses transactions bancaires.

## Phase 9 — V2 (à prioriser plus tard)

- Profils individuels : objectifs par membre (NUT-09), allergies et régimes (Q14), journal par personne
- ALI-10 à ALI-12 — partie comestible, cru ↔ cuit, micronutriments ; NUT-10
- REC-13, REC-14 — substitutions, extraction de recettes par IA
- COU-10, COU-11 — historique des prix, ticket de caisse
- CSH-05, CSH-06, TDB-05 — coûts croisés avec Cashflow, repas hors foyer
- STK-11 — statistiques de consommation
