# Spec 09 — Tableau de bord

Statut : **Revue** (2026-10-25)
Décisions : ADR-0010 (Cashflow), ADR-0012 (restes)

## Principe

Comme dans Cashflow : chaque chiffre affiché renvoie à **une définition écrite dans une spec**, et
l'interface dit comment il est calculé. Sans donnée, un encadré affiche « — » et ce qui manque, jamais
un chiffre inventé. Un total incomplet le dit (R-ALI-20).

L'accueil est fait d'**encadrés** que l'on réorganise et masque, comme l'accueil de Cashflow. Il est
conçu d'abord pour le téléphone : c'est la page qu'on ouvre en cuisine ou avant de partir en courses.

## Encadrés de l'accueil

| Encadré | Contenu | Définition |
|---|---|---|
| **Aujourd'hui** | Repas du jour : éléments, convives, heure de début conseillée, préparations utilisées ; bouton « valider » une fois le repas passé | spec 05, R-PLA-46 |
| **À faire** | Repas à valider, décongélations à faire, dates de péremption à compléter, rapprochements en attente | R-PLA-35, R-PLA-43, R-COU-51, R-CSH-30 |
| **À consommer en priorité** | Les premiers lots de la liste, avec leurs actions | R-GAS-1 à R-GAS-4 |
| **Restes** | Restes et préparations en stock, date limite, usage prévu | ADR-0012 |
| **Prochaines courses** | Date, magasin, nombre d'articles, total estimé, bouton « partir en courses » | R-COU-20, R-COU-25 |
| **Session de préparation** | Prochaine session : préparations, durée du planning, temps disponible | R-PLA-42 |
| **Bilan de la semaine** | Critères de l'objectif en vigueur : statut prévu et réel, les écarts d'abord | R-NUT-40 à R-NUT-44 |
| **Budget courses** | Prévu, réel, reste, projection de fin de mois (si le lien Cashflow est configuré) | R-CSH-11, R-CSH-12 |
| **Gaspillage du mois** | Valeur jetée, taux, comparaison au mois précédent | R-GAS-40 à R-GAS-42 |

- **R-TDB-1** — Un encadré dont la rubrique n'est pas utilisable (lien Cashflow non configuré, aucun
  objectif nutritionnel) est masqué, pas vide.
- **R-TDB-2** — Les comparaisons d'un mois en cours se font avec le mois précédent **à la même date**
  (même règle que R-TDB-13 de Cashflow).

## Pages d'analyse

| ID | Priorité | Contenu |
|---|---|---|
| TDB-01 | MVP | **Accueil** : encadrés Aujourd'hui, À faire, À consommer en priorité, Prochaines courses |
| TDB-02 | V1 | Encadrés Restes, Session de préparation, Bilan de la semaine, Budget courses, Gaspillage du mois ; réorganisation et masquage |
| TDB-03 | V1 | **Bilan nutritionnel** : semaine et mois, prévu et réel, détail par critère et par jour, incomplétude |
| TDB-04 | V1 | **Gaspillage** : statistiques de la spec 07 sur 12 mois |
| TDB-05 | V2 | **Coûts** : coût des repas, des courses et du gaspillage par mois, croisés avec Cashflow (CSH-05) |

### Critères d'acceptation

- Étant donné un lien Cashflow non configuré, alors l'encadré « Budget courses » n'apparaît pas
  (R-TDB-1).
- Étant donné un dîner d'hier ni validé ni sauté, alors il apparaît dans « À faire » avec l'action
  « valider » (R-PLA-35).
- Étant donné une semaine dont un repas a un ingrédient sans valeur de fibres, alors le bilan de la
  semaine affiche les fibres comme incomplètes, avec l'ingrédient en cause (R-NUT-24).
