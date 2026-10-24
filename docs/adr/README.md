# Architecture Decision Records

Un ADR documente **une** décision structurante : son contexte, les options envisagées, le choix
retenu et ses conséquences. Il sert à comprendre plus tard *pourquoi* le projet est comme il est.

## Processus

1. Copier [template.md](template.md) en `NNNN-titre-en-kebab-case.md` (numéro suivant).
2. Statut initial : `Proposé` (une option recommandée) ou `À décider` (plusieurs options, pas de choix).
3. Après revue par le propriétaire du projet : `Accepté`.
4. **Tant que l'application n'est pas utilisée** (aucune donnée réelle), un ADR, même `Accepté`, est
   modifié directement, options et décision comprises.
5. **Ensuite**, un ADR `Accepté` n'est plus modifié sur le fond : on crée un nouvel ADR, et l'ancien passe
   à `Remplacé par ADR-XXXX`. Seules les corrections de forme sont permises.
6. Mettre à jour l'index ci-dessous.

## Index

| N° | Titre | Statut |
|---|---|---|
| [0001](0001-quantites-en-entiers.md) | Quantités, nutriments et prix stockés en entiers | Accepté |
| [0002](0002-stock-calcule-par-mouvements.md) | Stock calculé à partir d'un journal de mouvements | Accepté |
| [0003](0003-aliments-familles-et-produits.md) | Aliments, familles d'aliments et produits | Accepté |
| [0004](0004-foyer-convives-et-preferences.md) | Foyer, convives et préférences | Accepté |
| [0005](0005-validation-explicite-des-mouvements.md) | Validation explicite des mouvements et des imports | Accepté |
| [0006](0006-mono-utilisateur.md) | Mono-utilisateur | Accepté |
| [0007](0007-services-tiers-optionnels.md) | Services tiers explicites et optionnels | Accepté |
| [0008](0008-export-ouvert.md) | Export complet dans un format ouvert | Accepté |
| [0009](0009-stack-et-deploiement.md) | Stack et déploiement : ceux de Cashflow, plus une PWA | Accepté |
| [0010](0010-lecture-de-cashflow.md) | Foodflow lit Cashflow par son API | Accepté |
| [0011](0011-temps-disponible-et-preparation-a-l-avance.md) | Temps disponible et préparation à l'avance (batch cooking) | Accepté |
| [0012](0012-gestion-des-restes.md) | Gestion des restes | Accepté |
| [0013](0013-generation-deterministe-des-menus.md) | Génération déterministe des menus | Proposé |
| [0014](0014-generation-par-ia.md) | Génération de menus par IA | Proposé |
