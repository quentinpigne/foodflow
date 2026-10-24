# ADR-0010 — Foodflow lit Cashflow par son API

- **Statut** : Accepté (2026-10-25)
- **Date** : 2026-10-24
- **Concerne** : `CSH`, `COU`, `TDB`, ADR-0007, ADR-0009 ; Cashflow (authentification, API)

## Contexte

Le budget alimentation est suivi dans Cashflow (lignes de budget sur les catégories « Alimentation »,
« Supermarché / épicerie »…). Foodflow connaît le coût prévu des repas et le montant de chaque session
de courses. Relier les deux permet :

- de planifier une semaine en voyant **ce qui reste du budget courses du mois** ;
- de **rapprocher une session de courses de la transaction bancaire** correspondante.

Les deux applications restent distinctes : chacune a sa base et ses règles.

Cashflow n'offre aujourd'hui qu'une session par cookie liée au mot de passe : aucun accès n'est prévu
pour une autre application.

## Options envisagées

1. **Base partagée** — ❌ couple les schémas, contourne les règles de Cashflow.
2. **Échange de fichiers (export/import)** — ✅ découplé / ❌ manuel, toujours en retard.
3. **Foodflow lit l'API de Cashflow, en lecture seule** — ✅ données à jour, Cashflow reste maître de ses
   données / ❌ demande un accès machine dans Cashflow.
4. **Cashflow lit Foodflow** — ❌ inverse le besoin : c'est en planifiant qu'on veut voir le budget.

## Décision

Option 3, décidée par le propriétaire le 2026-10-24.

- **Foodflow n'écrit jamais dans Cashflow.**
- Côté Cashflow : un **jeton d'accès en lecture seule** pour une application tierce, créé et révoqué
  depuis les réglages de Cashflow, haché en base, limité à une liste de routes de lecture
  (catégories, budgets du mois, recherche de transactions). Il fera l'objet d'un ADR **dans Cashflow**.
- Côté Foodflow : l'adresse de Cashflow et le jeton sont dans la configuration du serveur. Les appels
  partent du serveur Foodflow, jamais du navigateur. Le lien est **optionnel** : sans configuration,
  la rubrique est masquée.
- Foodflow mémorise les **identifiants Cashflow** qu'il a rattachés (catégories suivies, transaction
  rapprochée d'une session de courses), jamais les montants : ils sont relus à la demande.
- Une indisponibilité de Cashflow n'empêche rien dans Foodflow : seule la rubrique liée affiche
  « Cashflow injoignable ».

## Conséquences

- Travail à prévoir dans Cashflow : jeton d'accès, routes de lecture stables, avant la phase `CSH` de
  Foodflow.
- Les montants sont en centimes des deux côtés (ADR-0001) : aucune conversion.
- Le rapprochement est une proposition (même montant, date proche, tiers d'enseigne), validée à la main
  (ADR-0005).
