# ADR-0007 — Services tiers explicites et optionnels

- **Statut** : Accepté (2026-10-25)
- **Date** : 2026-10-24
- **Concerne** : `ALI`, `REC`, `PLA`, ADR-0009, ADR-0010

## Contexte

Plusieurs fonctions gagnent à appeler un service extérieur :

- **Open Food Facts** : décrire un produit à partir de son code-barres ;
- **sites de recettes** : télécharger une page pour en extraire la recette ;
- **IA (API Claude)** : proposer des menus à partir des recettes, du stock et des objectifs ;
- **notifications push** (Web Push) : prévenir le téléphone de ce qui périme, via le service de
  notification de son navigateur (spec 07).

Les données du foyer (stock, habitudes, goûts) sont personnelles. Les appels sortants doivent être
connus, maîtrisés et désactivables, comme l'OCR dans Cashflow (ADR-0007 de Cashflow).

## Options envisagées

1. **Appels libres, activés par défaut** — ❌ fuite de données implicite, dépendance au réseau.
2. **Aucun service tiers** — ❌ prive de fonctions utiles (scan, import, IA).
3. **Services explicites et optionnels** — ✅ utiles quand on les veut, l'application marche sans.

## Décision

Option 3.

- Chaque service a un **interrupteur** dans les réglages. Open Food Facts et l'import par URL sont activés
  par défaut (ils n'envoient qu'un code-barres ou une URL) ; **l'IA est désactivée par défaut** et
  demande une clé d'API.
- **Chaque appel est déclenché par une action de l'utilisateur** (scanner, importer, demander des menus),
  jamais en tâche de fond.
- Pour l'IA, **les données envoyées sont affichées avant l'envoi** et limitées au nécessaire : recettes
  candidates (titre, ingrédients, valeurs calculées), stock pertinent, objectif, préférences agrégées,
  sans nom de membre.
- **La réponse d'un service tiers est une proposition** soumise à validation (ADR-0005) ; une proposition
  de menu par l'IA est contrôlée par le domaine (recettes existantes, préférences, objectifs) avant
  affichage.
- **Ciqual est importée en local** et n'est pas un service tiers à l'exécution.
- Les réponses d'Open Food Facts sont mises en cache dans la base : un produit déjà connu ne déclenche
  plus d'appel.

## Conséquences

- Hors connexion à Internet, tout fonctionne sauf le scan d'un produit inconnu, l'import par URL et l'IA.
- Les clés d'API sont dans la configuration du serveur, jamais dans le dépôt ni dans la base exportée.
- Le choix du modèle et le format d'échange avec l'IA feront l'objet d'une spec `PLA` dédiée.
