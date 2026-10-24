# ADR-0012 — Gestion des restes

- **Statut** : Accepté (2026-10-25)
- **Date** : 2026-10-25
- **Concerne** : `STK`, `REC`, `PLA`, `NUT`, `GAS`, ADR-0002, ADR-0005, ADR-0011

## Contexte

On ne finit pas toujours un plat. Les restes sont du stock comme un autre, mais ils se périment vite et
ne servent que s'ils sont replanifiés : mangés tels quels sur un autre créneau, ou transformés en autre
chose (le poulet rôti du dimanche devient une salade le lundi). On peut aussi en faire exprès :
cuisiner une fois pour manger deux fois.

Les restes faussent aussi le bilan nutritionnel si l'on considère que tout ce qui a été cuisiné a été
mangé.

## Options envisagées

### Ce que devient un reste
1. **Un lot de la recette, en portions, seulement** — ✅ simple / ❌ un reste de poulet rôti ne peut
   servir qu'à manger du « poulet rôti », pas à une recette qui demande 200 g de poulet cuit.
2. **Un lot de la recette ou un lot d'aliment, au choix** — ✅ un plat se mange tel quel (lot de
   recette), un produit de base cuit se réutilise comme aliment (lot de « poulet rôti, chair », qui
   existe dans Ciqual) / ❌ un choix de plus à la validation, proposé par la recette.

### Bilan nutritionnel
1. **Tout ce qui est cuisiné est mangé** — ❌ faux dès qu'il y a des restes.
2. **Consommé = cuisiné − restes − jeté**, et les restes comptent dans le repas où ils sont mangés —
   ✅ juste, et sans double compte.

## Décision

Option 2 dans les deux cas.

### Déclarer un reste
- À la **validation d'un repas cuisiné** (ADR-0005), on indique, pour chaque recette du repas, ce qui
  reste : en portions, ou « rien ». Le reste devient un **lot** (mouvement `LEFTOVER`, ADR-0002), rangé
  au frigo par défaut, ou au congélateur si la recette est congelable :
  - un **lot de préparation** de la recette, en portions : le plat se mangera tel quel ;
  - ou un **lot d'aliment**, si la recette déclare un **aliment de reste** (poulet rôti → « Poulet,
    viande rôtie »), en quantité de cet aliment : il servira d'ingrédient à d'autres recettes.
- Sa date limite suit R-STK-34 (conservation au frigo de la recette, ou durée générale).
- Un reste peut aussi être déclaré **plus tard** (on s'aperçoit le lendemain qu'il reste des
  lasagnes) : c'est une correction de la validation du repas.

### Restes prévus
- Un repas peut être **cuisiné pour plus** que ses convives (« cuisiner une fois, manger deux fois ») :
  on fixe les portions cuisinées, et la différence est un **reste prévu**. La liste de courses compte les
  portions cuisinées.
- Un reste prévu doit être **affecté** à un repas ultérieur avant sa date limite, sinon la planification
  le signale. C'est le même mécanisme que la préparation à l'avance (ADR-0011), sans session dédiée.

### Utiliser un reste
Après la validation d'un repas qui laisse des restes, ou depuis le stock, l'application propose :
- **le finir tel quel** sur un créneau à venir, avant sa date limite, comme plat du repas ou en
  complément si les portions ne suffisent pas aux convives ;
- **le transformer** : les recettes qui l'utilisent, soit parce qu'elles ont la recette d'origine comme
  sous-recette (« Hachis parmentier » ← « Pot-au-feu », R-REC-32), soit parce qu'elles demandent l'aliment
  de reste ;
- **le congeler**, si la recette est congelable ;
- **le jeter** (`WASTE`), ce qui compte dans le gaspillage.

La planification, manuelle ou générée, **met en priorité les restes en stock** avant leur date limite.
Manger un reste ne compte pas comme une répétition de la recette pour les critères de variété
(spec `NUT`).

### Bilan nutritionnel
- Un repas compte ce qui y a été **réellement mangé** : quantités cuisinées − restes déclarés.
- Un repas de restes compte les portions de reste mangées, aux valeurs par portion de la recette, ou la
  quantité d'aliment de reste utilisée.

## Conséquences

- La recette gagne un champ facultatif : son aliment de reste.
- La validation d'un repas pose une question de plus (« il en reste ? »), répondue par défaut « rien »
  sauf pour un repas cuisiné pour plus.
- Le tableau de bord et la vue du stock affichent les restes à finir, triés par date limite.
- Règles détaillées : spec 02 (lots), spec 03 (aliment de reste), spec `PLA` (affectation, priorité),
  spec 04 (bilan).
