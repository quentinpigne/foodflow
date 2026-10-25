# Spec 08 — Lien avec Cashflow

Statut : **Brouillon** (2026-10-25)
Modèle : [CashflowCategory, TripPayment, Store](../modele-de-donnees.md#lien-avec-cashflow--spec-08)
Décisions : ADR-0001 (centimes), ADR-0005 (validation), ADR-0010 (lecture de Cashflow par son API),
ADR-0013 (plafond de coût)

## Objectif

Relier ce que Foodflow **prévoit** (repas, listes de courses) à ce que Cashflow **constate** (budget,
transactions bancaires), sans que l'une des applications écrive dans l'autre (ADR-0010) :

- planifier en sachant **ce qui reste du budget courses** du mois ;
- **rapprocher** chaque session de courses de la ou des transactions qui l'ont payée.

```
Budget courses — novembre 2026                     (Cashflow : « Supermarché / épicerie »)
───────────────────────────────────────────────────────────────────────────────────────────
Prévu 600,00 €   Réel 388,20 €   Reste 211,80 €
Courses prévues d'ici la fin du mois (Foodflow) :  samedi 21 ≈ 92 €, samedi 28 ≈ 85 € (incomplet)
Projection fin de mois : 565,20 € ✔ sous le prévu
Plafond proposé pour la génération de la semaine du 23 : 98 €

Session du samedi 14/11 — Carrefour Market, payé 87,40 €
  ✔ Rapprochée : CB CARREFOUR MARKET 16/11  −87,40 €  (Courant LCL)
```

## Prérequis côté Cashflow

Cashflow n'a aujourd'hui qu'une session par cookie liée au mot de passe. Le lien suppose, **dans
Cashflow**, un ADR et une évolution :

- des **jetons d'accès** créés et révoqués depuis les réglages de Cashflow, affichés une seule fois,
  stockés hachés, avec un nom (« Foodflow ») et une date de dernier usage ;
- envoyés dans l'en-tête `Authorization: Bearer <jeton>` ;
- en **lecture seule**, limités à une liste de routes : `GET /api/v1/categories`, `GET /api/v1/budget`,
  `GET /api/v1/transactions`, `GET /api/v1/transactions/{id}`, `GET /api/v1/payees`, `GET /api/v1/accounts` ;
  toute autre route répond 403 ;
- ces routes deviennent un **contrat stable** pour Foodflow : leur évolution incompatible passe par une
  nouvelle version de l'API.

## Règles

### Configuration

- **R-CSH-1** — Le lien est **optionnel** (ADR-0010) : sans adresse ni jeton dans la configuration du
  serveur, la rubrique est masquée et rien n'appelle Cashflow.
- **R-CSH-2** — Les appels partent **du serveur Foodflow**, jamais du navigateur. Le jeton n'est jamais
  transmis à l'interface ni exporté.
- **R-CSH-3** — Un **test de connexion** vérifie l'adresse, le jeton et ses droits, et dit précisément ce
  qui ne va pas (injoignable, jeton refusé, route interdite).
- **R-CSH-4** — Si Cashflow est injoignable, seules les vues du lien affichent « Cashflow injoignable » ;
  le reste de Foodflow fonctionne normalement. Aucune donnée de Cashflow n'est mise en cache au-delà de
  la durée d'un affichage.

### Catégories suivies et budget

- **R-CSH-10** — On choisit les **catégories de Cashflow suivies** comme « courses alimentaires »
  (par défaut proposée : « Supermarché / épicerie »). Foodflow ne mémorise que leurs identifiants
  (ADR-0010).
- **R-CSH-11** — Le **budget du mois** affiche, pour les catégories suivies, le prévu, le réel et le reste
  lus dans Cashflow (`GET /api/v1/budget`) ; une catégorie suivie sans ligne de budget dans Cashflow
  affiche son réel seulement.
- **R-CSH-12** — La **projection de fin de mois** = réel de Cashflow + total estimé des sessions de courses
  `PLANNED` restantes du mois (R-COU-25). Elle est incomplète si un total estimé l'est. Une session
  `DONE` mais pas encore passée en banque n'est pas comptée deux fois : si elle n'est pas rapprochée,
  son total payé s'ajoute au réel et elle est signalée « en attente de la banque ».
- **R-CSH-13** — Le **plafond proposé** pour une génération (ADR-0013) = reste du budget − totaux estimés
  des autres sessions prévues du mois, ramené à la période générée au prorata des jours. Il est
  modifiable avant de lancer la génération ; sans budget, aucun plafond n'est proposé.
- **R-CSH-14** — Le coût des repas (consommation, R-REC-53) et le budget (achats) **ne se comparent pas
  directement** : la projection s'appuie sur les achats prévus, comme Cashflow.

### Magasins et tiers

- **R-CSH-20** — Un magasin peut être relié à un **tiers** ou un **groupe de tiers** de Cashflow
  (« Carrefour Market » → groupe « Carrefour ») ; ce lien sert au rapprochement.

### Rapprochement des courses (ADR-0005)

- **R-CSH-30** — Pour une session `DONE`, Foodflow cherche dans Cashflow les **transactions candidates** :
  dépenses sur les comptes non clôturés, datées du jour de la session à 5 jours après (délai de passage
  en banque, comme la tolérance de Cashflow), non déjà rapprochées d'une autre session.
- **R-CSH-31** — Les candidates sont classées : montant égal au total payé, tiers ou groupe du magasin,
  proximité de la date ; puis par identifiant. Une candidate de montant égal et de tiers correspondant
  est proposée en premier, avec la mention « correspondance probable ».
- **R-CSH-32** — Le rapprochement est **toujours confirmé** par l'utilisateur. Une session peut être payée
  par **plusieurs transactions** (carte et titres-restaurant) : leur somme doit égaler le total payé,
  sinon l'écart est affiché et la confirmation demandée explicitement.
- **R-CSH-33** — Sans total payé, choisir une transaction **propose** son montant comme total payé.
- **R-CSH-34** — Une transaction de Cashflow est rapprochée d'**une seule** session. Le rapprochement se
  défait.
- **R-CSH-35** — Foodflow ne mémorise que l'**identifiant** des transactions rapprochées ; montant, date
  et libellé sont relus à l'affichage (ADR-0010). Une transaction supprimée dans Cashflow rend le
  rapprochement **orphelin**, signalé.
- **R-CSH-36** — Une session payée en espèces, ou sur un compte non suivi par Cashflow, peut être marquée
  **sans transaction** : elle ne sera plus proposée au rapprochement.

## Features

| ID | Priorité | Feature |
|---|---|---|
| CSH-00 | V1 | **Dans Cashflow** : jetons d'accès en lecture seule, routes autorisées, ADR dédié |
| CSH-01 | V1 | Configuration du lien, test de connexion, catégories suivies |
| CSH-02 | V1 | Budget courses du mois : prévu, réel, reste, projection de fin de mois |
| CSH-03 | V1 | Plafond de coût proposé à la génération de menus |
| CSH-04 | V1 | Magasins reliés aux tiers de Cashflow ; rapprochement des sessions de courses, paiements multiples, sans transaction |
| CSH-05 | V2 | Bilan mensuel croisé : dépenses alimentaires (Cashflow), coût des repas mangés, valeur gaspillée, coût réel par repas et par équivalent adulte |
| CSH-06 | V2 | Repas hors foyer : rapprochement avec les dépenses de restaurant de Cashflow |

### Critères d'acceptation

- Étant donné un lien non configuré, alors aucune vue ne mentionne Cashflow et aucun appel n'est fait
  (R-CSH-1).
- Étant donné Cashflow arrêté, alors la vue budget affiche « Cashflow injoignable » et la planification,
  le stock et les courses fonctionnent (R-CSH-4).
- Étant donné un prévu de 600 €, un réel de 388,20 € et deux sessions prévues estimées à 92 € et 85 €,
  alors la projection de fin de mois est de 565,20 € (R-CSH-12).
- Étant donné un reste de 211,80 €, une autre session prévue estimée à 85 € et une génération sur 7 des
  15 jours restants du mois, alors le plafond proposé est de 59,17 € (R-CSH-13).
- Étant donné une session du samedi payée 87,40 € chez Carrefour Market, relié au groupe « Carrefour »,
  et une transaction « CB CARREFOUR MARKET » de −87,40 € le lundi, alors elle est proposée en premier
  comme correspondance probable, et rien n'est enregistré avant ma confirmation (R-CSH-31, R-CSH-32).
- Étant donné une session de 95 € payée 70 € par carte et 25 € en titres-restaurant, alors je peux
  rapprocher les deux transactions, dont la somme égale le total payé (R-CSH-32).
- Étant donné une transaction rapprochée puis supprimée dans Cashflow, alors la session est signalée
  « rapprochement orphelin » (R-CSH-35).
