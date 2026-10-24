# ADR-0002 — Stock calculé à partir d'un journal de mouvements

- **Statut** : Accepté (2026-10-25)
- **Date** : 2026-10-24
- **Concerne** : `STK`, `COU`, `PLA`, `GAS`, modèle de données

## Contexte

Le stock change pour de nombreuses raisons : courses, repas cuisiné, restes, produit jeté, ouverture
d'un paquet, congélation, oubli constaté en ouvrant le frigo. Si l'on ne stocke que l'état courant,
on perd l'historique qui permet de savoir ce qu'on jette, ce qu'on consomme par mois, ou d'annuler une
validation de courses erronée. Cashflow a fait le même choix pour le solde (ADR-0002 de Cashflow).

## Options envisagées

1. **État corrigé sur place** (quantité courante par lot) — ✅ simple / ❌ aucun historique, annulation
   impossible, pas de statistique de gaspillage.
2. **Journal de mouvements, état calculé** — ✅ historique complet, annulation par mouvement inverse,
   statistiques / ❌ calcul à chaque lecture (négligeable aux volumes d'un foyer).
3. **Journal et état matérialisé** — ✅ lectures rapides / ❌ deux vérités à synchroniser.

## Décision

Option 2. Le stock est organisé en **lots** (un produit ou un aliment, un emplacement, une date de
péremption éventuelle). Chaque lot porte un **journal de mouvements** datés et typés :

| Type | Origine | Signe |
|---|---|---|
| `PURCHASE` | Validation d'une session de courses | + |
| `COOKED` | Validation d'un repas cuisiné | − |
| `LEFTOVER` | Reste d'un repas remis en stock (crée un lot) | + |
| `WASTE` | Produit jeté (péremption, abîmé) | − |
| `MOVE` | Changement d'emplacement (congélation) | − sur l'ancien lot, + sur le nouveau |
| `ADJUSTMENT` | Correction manuelle | ± |
| `INVENTORY` | Inventaire : constat de la quantité réelle | point de contrôle |

- **La quantité d'un lot se calcule** : somme des mouvements depuis le dernier inventaire, plus la
  quantité constatée par cet inventaire.
- Un **inventaire** est un point de contrôle, comme ceux de Cashflow : il fixe la quantité à une date,
  et l'écart avec la quantité calculée est conservé (il mesure la dérive du suivi).
- Un mouvement validé n'est pas modifié : on l'annule par un mouvement inverse, rattaché à l'original.
- Pour un produit suivi **en présence** (ADR-0003), les mouvements ne portent pas de quantité mais un
  état (`AVAILABLE`, `LOW`, `OUT`). Le même journal s'applique.

## Conséquences

- On peut répondre à « combien de lait a-t-on jeté ce trimestre ? » et annuler une validation.
- Les quantités d'un lot ne sont jamais négatives : une sortie supérieure au stock calculé est bornée
  et signale l'écart, qui invite à un inventaire.
- Le calcul doit rester rapide sur des années d'historique : un index par lot suffit ; un lot épuisé
  depuis longtemps peut être archivé.
- Règles détaillées dans la spec `STK`.
