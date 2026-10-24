# ADR-0003 — Aliments, familles d'aliments et produits

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : `ALI`, `STK`, `REC`, `COU`, `PLA`, modèle de données

## Contexte

Trois niveaux de description d'un aliment coexistent :

- une recette demande « 200 g de lait » ou « 100 g de fromage râpé » (n'importe lequel) ;
- Ciqual décrit des aliments génériques (« Lait demi-écrémé, UHT ») avec leurs valeurs nutritionnelles ;
- le placard contient « Lactel demi-écrémé 1 L », code-barres 3428…, décrit par Open Food Facts.

Pour calculer ce qui manque, il faut savoir qu'une bouteille de Lactel **satisfait** le besoin de lait
d'une recette, et qu'un emmental râpé satisfait un besoin de « fromage râpé ».

## Options envisagées

1. **Un seul niveau : le produit** — ❌ une recette ne peut pas demander « du lait » sans marque.
2. **Aliment générique et produit, sans famille** — ✅ couvre le cas courant / ❌ « un fromage à pâte
   dure, au choix » ne s'exprime pas.
3. **Trois niveaux : famille, aliment, produit** — ✅ couvre substitutions et préférences / ❌ un niveau
   de plus à maintenir.

## Décision

Option 3.

```
Famille d'aliments   « Fromages à pâte pressée cuite »   (arborescence : Produits laitiers > Fromages > …)
  └─ Aliment         « Emmental »                          valeurs nutritionnelles (Ciqual ou saisie)
       └─ Produit    « Emmental râpé Président 200 g »    code-barres, conditionnement (Open Food Facts ou saisie)
```

- **Famille d'aliments** (`FoodGroup`) : arborescence. Elle porte aussi le rattachement aux groupes
  d'aliments des repères nutritionnels (légumes, fruits, féculents, légumineuses…) utilisé par `NUT`.
- **Aliment** (`Food`) : générique, avec ses valeurs nutritionnelles pour 100 g ou 100 mL et ses données
  de conversion (densité, poids moyen d'une pièce). Il appartient à une famille.
- **Produit** (`Product`) : article du commerce, avec code-barres et conditionnement. Il est **toujours
  rattaché à un aliment ou, à défaut, à une famille**. Ses valeurs nutritionnelles propres, si elles sont
  connues, priment sur celles de l'aliment.
- Un **ingrédient de recette** désigne un aliment **ou** une famille (« fromage râpé au choix »).
- Un **lot de stock** contient un produit **ou** un aliment (vrac, marché, fait maison).
- Le **mode de suivi** du stock (quantité ou présence) se règle par aliment, avec une valeur par défaut
  héritée de sa famille (épices et condiments : présence).

**Satisfaction d'un besoin** : un lot satisfait un ingrédient si son aliment est celui demandé, ou si
son aliment appartient à la famille demandée (ou à l'une de ses sous-familles).

## Conséquences

- La liste de courses s'exprime en aliments (« lait, 1 L »), avec le produit habituel proposé.
- Les goûts du foyer (ADR-0004) peuvent viser une famille entière (« pas d'abats »).
- L'import Ciqual alimente les aliments et une première arborescence de familles ; les familles restent
  modifiables.
- Un produit scanné inconnu exige un rattachement avant d'entrer en stock (suggestion à partir des
  catégories Open Food Facts, validation manuelle).
