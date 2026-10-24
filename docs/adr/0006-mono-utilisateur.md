# ADR-0006 — Mono-utilisateur

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : architecture, ADR-0004, ADR-0009

## Contexte

Une seule personne saisit et pilote l'application pour tout le foyer. Les membres du foyer (ADR-0004)
sont des données (convives, préférences), pas des utilisateurs qui se connectent.

## Options envisagées

1. **Mono-utilisateur, sans préparation** — ❌ ajouter des comptes plus tard toucherait toute l'application.
2. **Mono-utilisateur, préparé comme Cashflow** (une table `User`, des sessions qui y sont rattachées) —
   ✅ coût minime, ouverture possible.
3. **Multi-utilisateur dès la V1** — ❌ sans besoin identifié.

## Décision

Option 2, comme Cashflow (ADR-0006 de Cashflow) : un utilisateur unique, protégé par mot de passe, avec
les mêmes modalités d'authentification (ADR-0009). Un `Member` du foyer n'est **pas** un utilisateur ;
on pourra plus tard relier un utilisateur à un membre.

## Conséquences

- Pas de droits ni de partage à gérer en V1.
- Les données appartiennent au foyer, pas à un utilisateur : aucune table métier ne porte de `user_id`.
