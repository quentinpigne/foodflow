# ADR-0004 — Foyer, convives et préférences

- **Statut** : Proposé
- **Date** : 2026-10-24
- **Concerne** : `PLA`, `NUT`, `REC`, `COU`, modèle de données, ADR-0006

## Contexte

Les repas se planifient pour le foyer entier, avec un objectif global. Mais :

- un enfant ne mange pas autant qu'un adulte : les portions et les calories doivent en tenir compte ;
- certains repas comptent plus de convives que d'habitude (invités), ou moins (un membre absent) ;
- chacun a ses goûts, que la génération de menus doit respecter ;
- plus tard (V2), chacun pourra avoir ses objectifs, ses allergies et son journal alimentaire.

## Options envisagées

1. **Un nombre de portions par repas** — ✅ minimal / ❌ ni goûts individuels, ni enfants, ni V2.
2. **Membres du foyer avec coefficient et préférences, objectif nutritionnel au niveau du foyer** —
   ✅ couvre la V1 et prépare la V2 / ❌ un peu plus de saisie au départ.
3. **Profils individuels complets dès la V1** — ❌ hors périmètre V1.

## Décision

Option 2.

- **Membre du foyer** (`Member`) : un nom, un **coefficient d'équivalent adulte** (1,000 pour un adulte,
  0,600 pour un jeune enfant, réglable) et ses **préférences**. Il est présent par défaut aux repas
  selon un rythme réglable (par exemple : absent le midi en semaine).
- **Convives d'un repas** : les membres présents par défaut, modifiables pour ce repas (absent,
  présent), plus des **invités** ajoutés ponctuellement, chacun avec un coefficient (anonymes en V1).
- **Équivalents adulte d'un repas** = somme des coefficients de ses convives. Il détermine les quantités
  cuisinées (mise à l'échelle des recettes) et la part du repas dans l'objectif nutritionnel.
- **Préférences** (`Preference`) : par membre, sur un aliment, une famille d'aliments ou une recette,
  avec un niveau : `LOVES`, `LIKES`, `DISLIKES`, `REFUSES`. La génération exclut ce qu'un convive refuse
  et favorise ce que les convives aiment. Les préférences d'un invité ne sont pas saisies en V1.
- **L'objectif nutritionnel** (spec `NUT`) est défini pour **un équivalent adulte** et par jour ;
  le bilan d'une période le compare aux apports ramenés à l'équivalent adulte.

## Conséquences

- La V2 (profils nutritionnels, allergies, régimes, journal par personne) s'ajoute au `Member` existant
  sans reprendre le modèle. Les allergies n'utiliseront pas les préférences : elles seront des contraintes
  dures, avec leurs propres règles.
- Les invités exceptionnels augmentent les quantités et donc la liste de courses de la semaine.
- Un repas sans convive (tout le monde absent) n'est pas cuisiné et ne compte ni dans le stock ni dans
  les courses.
