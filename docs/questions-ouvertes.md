# Questions ouvertes

Statut : **vivant** — on retire une question dès qu'elle est tranchée, en indiquant où la réponse
a été consignée (ADR, spec).

## Bloquantes (phase 0)

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q1 | Foyer : comment compter les convives ? Un nombre de personnes, ou des « équivalents adulte » (un enfant = 0,6) pour les portions et les calories ? | Équivalents adulte, réglables par repas | vision, `PLA` |
| Q2 | Structure de la journée : quels repas (petit-déjeuner, déjeuner, goûter, dîner) ? Les repas pris hors du foyer (cantine, restaurant) comptent-ils dans l'objectif ? | Créneaux réglables ; un repas « hors foyer » occupe le créneau sans compter dans le stock ni dans les courses | `PLA`, `NUT` |
| Q3 | Lien avec Cashflow : dans quel sens, et comment ? Foodflow lit le budget Alimentation de Cashflow ? Rapproche une session de courses d'une transaction Cashflow ? Cashflow lit les coûts de Foodflow ? | Foodflow appelle l'API de Cashflow (lecture du budget, recherche de la transaction des courses) ; aucune écriture dans Cashflow | `CSH`, ADR |
| Q4 | Stack et déploiement : la même que Cashflow (Rust axum + sqlx/SQLite, Angular, binaire unique, Docker sur ubuntu01) ? | Oui, avec en plus une PWA pour le mode magasin hors connexion | ADR |
| Q5 | Précision des quantités et des valeurs nutritionnelles : entiers (milligrammes, millilitres, millièmes de pièce) comme les centimes de Cashflow ? | Oui, pas de flottant pour les quantités, les nutriments ni les prix | ADR |
| Q6 | Stock : un état qu'on corrige, ou un journal de mouvements (entrées, sorties, inventaires) dont l'état se déduit, comme le solde de Cashflow ? | Journal de mouvements avec inventaires comme points de contrôle | `STK`, ADR |
| Q7 | Aliment et produit : un produit du commerce (code-barres) est-il toujours rattaché à un aliment générique (« Lait demi-écrémé ») qui sert aux recettes et au stock ? | Oui : les recettes demandent un aliment, le stock contient des produits ou des aliments | `ALI`, ADR |

## À trancher avant la phase concernée

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q8 | IA : quel fournisseur, quelles données lui envoyer (stock, recettes, objectifs) ? | API Claude, désactivée par défaut, envoi limité au strict nécessaire et affiché avant l'envoi | `PLA`, ADR |
| Q9 | Open Food Facts : appel à la demande, ou copie locale filtrée ? | Appel à la demande au scan, résultat mis en cache dans la base | `ALI` |
| Q10 | Prix : saisis article par article aux courses, ou total du ticket seulement ? | Prix par article facultatif ; total de la session obligatoire pour le lien avec Cashflow | `COU`, `CSH` |

## Tranchées

| # | Question | Réponse | Où |
|---|---|---|---|
| — | Pour qui planifier ? | Le foyer entier, sans profil individuel en V1, sans s'interdire d'en ajouter (2026-10-24) | vision |
| — | Précision du stock | Choisie par produit : quantité ou présence ; péremption suivie (2026-10-24) | vision, `STK` |
| — | Sources nutritionnelles | Ciqual, Open Food Facts, saisie manuelle (2026-10-24) | vision, `ALI` |
| — | Construction des menus | Saisie manuelle, génération déterministe, génération par IA optionnelle (2026-10-24) | vision, `PLA` |
| — | Sources de recettes | Saisie manuelle, import depuis une URL (2026-10-24) | vision, `REC` |
| — | Critères d'équilibre | Calories, macronutriments, groupes PNNS, variété ; réglables et versionnés (2026-10-24) | vision, `NUT` |
| — | Mise à jour du stock | Validation explicite après les courses et après chaque repas cuisiné (2026-10-24) | vision, `STK` |
| — | Usages des courses | Mode magasin hors connexion, prix et coût, anti-gaspillage, lien avec les budgets de Cashflow (2026-10-24) | vision |
