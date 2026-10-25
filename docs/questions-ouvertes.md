# Questions ouvertes

Statut : **vivant** — on retire une question dès qu'elle est tranchée, en indiquant où la réponse
a été consignée (ADR, spec).

## Bloquantes (phase 0)

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| — | *Aucune.* | — | — |

## À trancher avant la phase concernée

| # | Question | Proposition par défaut | Où |
|---|---|---|---|
| Q16 | Certificat HTTPS : le domaine est `pigne.ddns.net` (No-IP, 2026-10-25), mais No-IP ne propose pas d'API pour les enregistrements TXT (pas de fournisseur lego) ni de sous-domaine sur un hôte gratuit : le challenge DNS-01 automatique n'est pas possible. Alternatives : Duck DNS (gratuit, fournisseur lego `duckdns`, adresse privée possible), domaine personnel chez un registraire avec API (≈ 10 €/an), challenge HTTP-01 (ouvre le port 80 sur Internet, contraire à ADR-0009), autorité de certification interne (option 2 de ADR-0009) | Duck DNS | ADR-0009, phase 1 |

## Pour plus tard (V2)

| # | Question | Où |
|---|---|---|
| Q14 | Allergies et régimes : liste fermée (14 allergènes réglementaires) ou libre ? Sur l'aliment, la famille ou le produit (traces) ? | ADR-0004 |
| Q15 | Préférences des invités récurrents : un invité devient-il un membre « occasionnel » ? | ADR-0004 |

## Tranchées

| # | Question | Réponse | Où |
|---|---|---|---|
| Q18 | Semaine type par défaut | Semaine 10 / 20 / 30 min, week-end 20 / 30 / 45 min ; aucune session de préparation par défaut (2026-10-25) | ADR-0011 |
| — | Date de péremption dépassée | L'utilisateur décide : jeter, ou garder avec une nouvelle date limite (2026-10-25) | spec 02, R-STK-37 |
| — | Choix des specs 01 et 02 | Validés (2026-10-25) | specs 01, 02 |
| Q9 | Open Food Facts | Appel à la demande au scan, résultat gardé en base (2026-10-25) | ADR-0007, spec 01 |
| Q10 | Prix aux courses | Par article, facultatif ; total payé facultatif, nécessaire au rapprochement avec Cashflow (2026-10-25) | spec 06 |
| Q12 | Fournisseur d'IA | API compatible OpenAI (probablement derrière LiteLLM), modèle et plafond dans la configuration (2026-10-25) | ADR-0014 |
| — | Documents transverses et spec 09 | Validés ; phase 0 close (2026-10-25) | docs/ |
| — | ADR des jetons d'accès dans Cashflow | Écrit par le propriétaire dans le dépôt Cashflow (2026-10-25) | spec 08, CSH-00 |
| — | Spec 08 | Validée (2026-10-25) | spec 08 |
| — | Écrans | Implémentés directement en s'inspirant de Cashflow, corrigés visuellement ; pas de document d'écrans (2026-10-25) | architecture, roadmap |
| — | Spec 07 | Validée (2026-10-25) | spec 07 |
| — | Spec 06 | Validée (2026-10-25) | spec 06 |
| — | Notifications push | Dès la V1, Web Push chiffré, désactivable (2026-10-25) | spec 07, ADR-0007 |
| — | Spec 05, ADR-0013, ADR-0014 | Validés (2026-10-25) | spec 05 |
| — | Ordre de cuisson d'une session | Optimisé dès la V1 (2026-10-25) | ADR-0015 |
| — | Spec 04 | Validée (2026-10-25) | spec 04 |
| — | Composition d'un repas | Plusieurs éléments, composition type par créneau (2026-10-25) | spec 05 |
| — | Repas routiniers | Repas habituels repris par défaut (2026-10-25) | spec 05 |
| — | Rôle de l'IA | Compose avec les recettes existantes, peut proposer des recettes nouvelles en brouillon (2026-10-25) | ADR-0014 |
| Q13 | Créneaux par défaut et part de l'énergie | Petit-déjeuner 25 %, déjeuner 35 %, goûter 10 %, dîner 30 % (2026-10-25) | spec 04 |
| Q21 | Énergie de référence | 2 000 kcal/j par équivalent adulte, ± 10 % (2026-10-25) | spec 04 |
| — | Gestion des restes | ADR-0012 accepté (2026-10-25) | ADR-0012 |
| Q19 | Import par URL | JSON-LD seulement, échec franc sinon ; extraction IA en V2 (2026-10-25) | spec 03 |
| Q20 | Prix de référence d'un aliment | Dernier prix payé du produit habituel, sinon de n'importe quel produit de l'aliment (2026-10-25) | spec 03, `COU` |
| — | Choix de la spec 03 | Validés (2026-10-25) | spec 03 |
| Q17 | Granularité du batch cooking | Recette entière et sous-recettes, pas d'étape (2026-10-25) | ADR-0011 |
| Q8 | HTTPS pour la PWA | Domaine personnel, Let's Encrypt par challenge DNS-01, certificat renouvelé hors du binaire (2026-10-25) | ADR-0009 |
| Q11 | Revue des ADR 0001 à 0010 | Tous acceptés (2026-10-25) | adr/ |
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
