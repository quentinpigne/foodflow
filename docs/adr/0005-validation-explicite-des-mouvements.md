# ADR-0005 — Validation explicite des mouvements et des imports

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : `STK`, `COU`, `PLA`, `REC`, ADR-0002

## Contexte

Le stock n'est utile que s'il est fiable. Un décompte automatique (un repas planifié retire ses
ingrédients à sa date) dérive dès qu'on change d'avis, qu'on cuisine plus ou moins, ou qu'on remplace un
ingrédient. De même, une recette importée d'un site contient des ingrédients en texte libre
(« 2 belles gousses d'ail ») qu'il faut rattacher aux aliments avant de pouvoir calculer quoi que ce soit.

## Options envisagées

1. **Décompte automatique** — ✅ aucune saisie / ❌ stock faux, sans qu'on le voie.
2. **Corrections manuelles seulement** — ❌ trop de saisie : personne ne la fait.
3. **Propositions validées** — ✅ une seule action, avec des valeurs préremplies et ajustables.

## Décision

Option 3. Rien ne modifie le stock ni la base de recettes sans **validation explicite**.

- **Courses faites** : la session de courses propose les articles cochés comme entrées en stock
  (quantités, emplacement, péremption, prix) ; on ajuste, puis on valide. La validation crée les lots et
  les mouvements `PURCHASE`.
- **Repas cuisiné** : le repas propose de sortir les ingrédients de la recette, mis à l'échelle des
  convives, en puisant d'abord dans les lots qui périment le plus tôt ; on ajuste (quantités, lots,
  restes), puis on valide. Un repas planifié mais non validé ne touche pas le stock.
- **Import de recette** depuis une URL : l'import produit un brouillon dont chaque ingrédient est
  rattaché à un aliment ou une famille, avec un indice de confiance ; on revoit, puis on enregistre.
- Une validation s'annule : les mouvements créés sont annulés par des mouvements inverses (ADR-0002).

## Conséquences

- Le stock reste juste tant que les validations sont faites ; le tableau de bord signale les repas passés
  ni validés ni annulés.
- Valider doit être rapide : les valeurs proposées doivent être bonnes dans la majorité des cas
  (les quantités et lots préremplis, le dernier emplacement et le dernier prix connus du produit).
- Le rattachement d'un ingrédient importé à un aliment est mémorisé, pour les imports suivants.
