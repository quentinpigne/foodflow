# ADR-0001 — Quantités, nutriments et prix stockés en entiers

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : modèle de données, `ALI`, `STK`, `REC`, `COU`

## Contexte

L'application additionne et convertit sans cesse : ingrédients d'une recette mis à l'échelle des convives,
sorties de stock, besoins de la semaine moins le stock, valeurs nutritionnelles d'un menu, coût d'un repas.
Les flottants accumulent des erreurs d'arrondi (0,1 + 0,2 ≠ 0,3) qui finissent par se voir : une liste de
courses qui demande « 0,000001 kg de farine », un stock qui n'est jamais exactement à zéro.
Cashflow a fait le même choix pour l'argent (ADR-0001 de Cashflow).

## Options envisagées

1. **Flottants partout** — ✅ simple / ❌ erreurs d'arrondi, comparaisons à zéro fausses.
2. **Décimaux à précision arbitraire en base** (texte) — ✅ exacts / ❌ tris et sommes SQL impossibles,
   stockage verbeux.
3. **Entiers dans une unité fine, décimaux dans le calcul et l'affichage** — ✅ exacts, sommables en SQL,
   même discipline que Cashflow / ❌ conversions à centraliser.

## Décision

Option 3. **Aucun flottant n'est stocké ni utilisé dans le domaine** (clippy l'interdit, comme dans Cashflow).
L'interface affiche et saisit des décimaux (« 1,5 kg », « 2,49 € ») ; la conversion se fait à un seul
endroit côté serveur et côté client.

| Grandeur | Unité de stockage | Exemple |
|---|---|---|
| Masse | milligramme (mg) | 1,5 kg → `1 500 000` |
| Volume | microlitre (µL) | 1 c. à café (5 mL) → `5 000` |
| Pièce | millième de pièce | ½ citron → `500` |
| Énergie | calorie (cal, millième de kcal), pour 100 g ou 100 mL | 52 kcal → `52 000` |
| Nutriments (protéines, lipides, glucides, fibres, sucres, sel…) | mg pour 100 g ou 100 mL | 0,12 g → `120` |
| Prix | centime | 2,49 € → `249` |
| Prix au kilo, au litre ou à la pièce | centime par kg, par L ou par pièce | 12,90 €/kg → `1 290` |
| Coefficient (équivalent adulte, rendement) | millième | 0,6 → `600` |

Les calculs intermédiaires (mise à l'échelle d'une recette, coût d'une portion) se font en
`rust_decimal` ; **l'arrondi n'intervient qu'au moment de stocker ou d'afficher**, au plus proche.

## Conséquences

- Les sommes et comparaisons SQL sont exactes ; « stock nul » se teste par `= 0`.
- Les unités de cuisine (cuillère, pincée, verre, sachet) ne sont pas des unités de stockage : elles se
  convertissent vers l'une des trois grandeurs ci-dessus (spec `ALI`, unités et conversions).
- Passer d'une grandeur à une autre (pièce → masse, volume → masse) exige une donnée propre à l'aliment
  (poids moyen d'une pièce, densité) : sans elle, la conversion est impossible et signalée.
- Une `i64` en mg couvre largement les quantités domestiques (plus de 9 millions de tonnes).
