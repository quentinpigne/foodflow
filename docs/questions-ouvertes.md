# Questions ouvertes

Statut : **vivant** — on retire une question dès qu'elle est tranchée, en indiquant où la réponse
a été consignée (ADR, spec).

## Bloquantes (phase 0)

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q8 | HTTPS pour la PWA : autorité de certification interne (certificat à installer sur chaque appareil) ou domaine personnel avec Let's Encrypt par challenge DNS ? Tranche aussi Q15 de Cashflow | Domaine personnel, si tu en as un dont le DNS a une API | ADR-0009 |
| Q11 | Revue des ADR 0001 à 0010 (statut `Proposé`) | — | adr/ |

## À trancher avant la phase concernée

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q9 | Open Food Facts : appel à la demande, ou copie locale filtrée ? | Appel à la demande au scan, résultat mis en cache dans la base | ADR-0007, `ALI` |
| Q10 | Prix : saisis article par article aux courses, ou total du ticket seulement ? | Prix par article facultatif ; total de la session obligatoire pour le rapprochement avec Cashflow | `COU`, `CSH` |
| Q12 | IA : quel modèle, quel format d'échange, quel garde-fou de coût ? | API Claude, désactivée par défaut ; réponse structurée contrôlée par le domaine | ADR-0007, `PLA` |
| Q13 | Créneaux de repas par défaut (petit-déjeuner, déjeuner, goûter, dîner) et part de l'objectif journalier de chacun | 4 créneaux, 25 / 35 / 10 / 30 % | `PLA`, `NUT` |

## Pour plus tard (V2)

| # | Question | Où |
|---|---|---|
| Q14 | Allergies et régimes : liste fermée (14 allergènes réglementaires) ou libre ? Sur l'aliment, la famille ou le produit (traces) ? | ADR-0004 |
| Q15 | Préférences des invités récurrents : un invité devient-il un membre « occasionnel » ? | ADR-0004 |

## Tranchées

| # | Question | Réponse | Où |
|---|---|---|---|
| Q1 | Comment compter les convives ? | En équivalents adulte ; invités et absents réglables par repas (2026-10-24) | ADR-0004 |
| Q2 | Structure de la journée | Créneaux de repas paramétrables (2026-10-24) | vision, `PLA` |
| Q3 | Lien avec Cashflow | Foodflow lit Cashflow par son API, en lecture seule (2026-10-24) | ADR-0010 |
| Q4 | Stack et déploiement | Identiques à Cashflow, plus une PWA (2026-10-24) | ADR-0009 |
| Q5 | Précision des quantités | Entiers en base, décimaux dans l'interface (2026-10-24) | ADR-0001 |
| Q6 | Stock : état ou journal ? | Journal de mouvements, inventaires comme points de contrôle (2026-10-24) | ADR-0002 |
| Q7 | Aliment et produit | Un produit est toujours rattaché à un aliment générique ou à une famille (2026-10-24) | ADR-0003 |
| — | Allergies, conseils médicaux, journal par personne | V2 (2026-10-24) | vision |
| — | Goûts de chacun | En V1, par membre du foyer (2026-10-24) | ADR-0004 |
| — | Pour qui planifier ? | Le foyer entier, objectif global (2026-10-24) | vision, ADR-0004 |
| — | Précision du stock | Choisie par aliment : quantité ou présence ; péremption suivie (2026-10-24) | ADR-0003, `STK` |
| — | Sources nutritionnelles | Ciqual, Open Food Facts, saisie manuelle (2026-10-24) | vision, `ALI` |
| — | Construction des menus | Saisie manuelle, génération déterministe, génération par IA optionnelle (2026-10-24) | vision, `PLA` |
| — | Sources de recettes | Saisie manuelle, import depuis une URL (2026-10-24) | vision, `REC` |
| — | Critères d'équilibre | Calories, macronutriments, groupes PNNS, variété ; réglables et versionnés (2026-10-24) | vision, `NUT` |
| — | Mise à jour du stock | Validation explicite après les courses et après chaque repas cuisiné (2026-10-24) | ADR-0005 |
| — | Usages des courses | Mode magasin hors connexion, prix et coût, anti-gaspillage, lien avec les budgets de Cashflow (2026-10-24) | vision |
