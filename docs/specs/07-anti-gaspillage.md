# Spec 07 — Anti-gaspillage

Statut : **Validée** (2026-10-25)
Modèle : [Notification, PushSubscription, Setting](../modele-de-donnees.md#anti-gaspillage--spec-07)
Décisions : ADR-0002 (journal de mouvements), ADR-0007 (services tiers), ADR-0009 (PWA, HTTPS),
ADR-0012 (restes), ADR-0013 (génération)

## Objectif

Jeter moins, de trois façons : **voir à temps** ce qui va périmer, **savoir quoi en faire** (le cuisiner,
le congeler, le planifier), et **mesurer** ce qu'on jette pour s'améliorer. L'anti-gaspillage réunit des
règles déjà posées ailleurs (états de péremption, R-STK-35 ; date dépassée, R-STK-37 ; restes,
ADR-0012 ; priorité au stock dans la génération, ADR-0013) et ajoute les alertes, les suggestions et
les statistiques.

```
À consommer en priorité                                             lundi 09/11
──────────────────────────────────────────────────────────────────────────────
⚠ Yaourts nature (×3)       DLC dépassée hier          [Encore bon] [Jeter]
● Restes : lasagnes 2,5 p.  à finir avant mar. 10/11   prévu : —    [Planifier] [Congeler]
● Crème fraîche 12 cL       ouverte, à finir avant mer. 11/11  sans usage prévu
● Courgettes 600 g          SOON — mer. 11/11          prévu : gratin mar. ✔
❄ Chili (4 p.)              à décongeler ce soir pour le dîner de demain

Recettes pour finir la crème et les courgettes :
  1. Velouté de courgettes à la crème   utilise 100 % des courgettes et 10 cL de crème · 25 min
  2. Quiche courgettes                   utilise 400 g de courgettes et 12 cL de crème · 20 min + 35 min
```

## Règles

### À consommer en priorité

- **R-GAS-1** — La liste **« à consommer en priorité »** rassemble, triés par date limite effective :
  - les lots `SOON`, `EXPIRED` et `PAST_BEST_BEFORE` (R-STK-35), avec la question de R-STK-37 pour ces
    deux derniers ;
  - les lots de restes et de préparation, quelle que soit leur échéance ;
  - les lots dont la date de péremption est **à compléter** (R-COU-51) ;
  - les **décongélations** à faire pour un repas prévu (R-PLA-43).
- **R-GAS-2** — Le seuil « bientôt » (`SOON`) est un réglage général (3 jours par défaut) ; il peut être
  remplacé par emplacement (1 jour pour le frigo, 30 pour le placard, par exemple).
- **R-GAS-3** — Chaque lot de la liste indique s'il a un **usage prévu** : un repas ou une préparation
  planifiés qui le consomment avant sa date limite, selon la simulation de R-COU-10.
- **R-GAS-4** — Un lot est **sans usage prévu** si, en simulant les besoins planifiés jusqu'à sa date
  limite effective (R-COU-10), il lui reste une quantité à cette date : c'est la quantité qui sera
  perdue si rien ne change. Elle est affichée (« 12 cL sans usage prévu »).

### Agir sur un lot

- **R-GAS-10** — Depuis la liste, un lot propose les actions qui s'appliquent :
  - **planifier** : choisir un créneau à venir avant la date limite, avec une recette qui l'utilise, ou
    comme élément de repas (restes, aliment) ;
  - **congeler** (R-STK-32), si l'aliment ou la recette le permet et que la date limite n'est pas
    dépassée ;
  - **encore bon** ou **jeter** (R-STK-37) ;
  - **ouvrir**, **déplacer**, **corriger** la quantité (spec 02).
- **R-GAS-11** — Planifier depuis la liste crée ou complète un repas `PLANNED` comme une saisie manuelle
  (spec 05), avec ses signalements.

### Suggestions de recettes

- **R-GAS-20** — Pour un ou plusieurs lots choisis (par défaut : tous les lots sans usage prévu et
  `SOON`), l'application **classe les recettes** non archivées qui les utilisent :
  1. nombre de lots choisis utilisés ;
  2. part de la quantité de ces lots consommée (pour une portion adulte × les équivalents adulte du
     prochain créneau libre) ;
  3. part des autres ingrédients déjà couverte par le stock ;
  4. faisabilité au prochain créneau libre (temps, R-PLA-46) ;
  5. préférences des convives habituels ;
  puis par identifiant : le classement est déterministe.
- **R-GAS-21** — **« Que cuisiner avec ce que j'ai ? »** : la même recherche sans lot choisi, triée par
  part des ingrédients couverte par le stock actuel ; les recettes entièrement couvertes viennent en
  premier.
- **R-GAS-22** — Les recettes qui utilisent un **reste** comme sous-recette ou par son aliment de reste
  (ADR-0012) sont proposées pour ce reste.

### Alertes et notifications

- **R-GAS-30** — Les alertes sont calculées chaque jour par le serveur (tâche de fond, comme les
  sauvegardes de Cashflow) et à chaque mouvement de stock ; elles alimentent le **centre de
  notifications** de l'application.
- **R-GAS-31** — Types d'alertes : lot qui devient `SOON` ; lot dont la date est dépassée ; reste ou
  préparation à finir ; décongélation à faire ; lot sans usage prévu qui devient `SOON` ; date de
  péremption à compléter ; repas à valider (R-PLA-35).
- **R-GAS-32** — **Notifications push** (Web Push, PWA installée) :
  - un **résumé quotidien** à l'heure choisie (8 h par défaut) : ce qui périme aujourd'hui et demain,
    les restes à finir, les repas du jour ;
  - un **rappel de décongélation** la veille du repas, à l'heure choisie (20 h par défaut).

  Chaque type se désactive séparément. Une alerte déjà notifiée n'est pas renvoyée.
- **R-GAS-33** — Le push est un **service tiers** (ADR-0007) : les messages passent par le service de
  notification du navigateur du téléphone. Il est désactivable, le contenu est **chiffré** de bout en
  bout (Web Push, clés VAPID générées par le serveur) et réduit au strict nécessaire. Sans push,
  les alertes restent dans le centre de notifications.
- **R-GAS-34** — Les heures de notification s'entendent dans le **fuseau horaire du foyer** (réglage,
  `Europe/Paris` par défaut) ; c'est le seul usage d'un fuseau dans l'application.

### Statistiques

- **R-GAS-40** — Le **gaspillage** d'une période est la somme des mouvements `WASTE` (R-STK-13) : en
  quantité par aliment, et en **valeur** (prix du lot au prorata, R-COU-61 ; à défaut, prix de référence,
  marqué estimé ; sinon inconnu).
- **R-GAS-41** — Il se ventile par motif (`EXPIRED`, `SPOILED`, `OTHER`), par famille, par emplacement et
  par mois. Les restes jetés sont comptés à part, valorisés au coût par portion de la recette (estimé).
- **R-GAS-42** — **Taux de gaspillage** d'un mois = valeur jetée ÷ valeur achetée (mouvements `PURCHASE`)
  ; il est incomplet si des valeurs manquent.
- **R-GAS-43** — Les **écarts d'inventaire** négatifs (R-STK-40) sont présentés à part, comme « pertes
  inexpliquées » : ce n'est pas du gaspillage constaté, mais un signe que le suivi dérive.

## Features

| ID | Priorité | Feature |
|---|---|---|
| GAS-01 | MVP | Liste « à consommer en priorité », seuils par emplacement |
| GAS-02 | V1 | Usage prévu et lots sans usage prévu |
| GAS-03 | V1 | Actions depuis la liste : planifier, congeler, encore bon, jeter |
| GAS-04 | V1 | Suggestions de recettes pour finir des lots ; « que cuisiner avec ce que j'ai ? » |
| GAS-05 | V1 | Centre de notifications |
| GAS-06 | V1 | Notifications push : résumé quotidien, rappel de décongélation |
| GAS-07 | V1 | Statistiques de gaspillage : quantités, valeur, motifs, taux, pertes inexpliquées |

### Critères d'acceptation

- Étant donné 20 cL de crème ouverte à finir avant mercredi et un repas prévu mardi qui en utilise 8 cL,
  alors la crème apparaît avec « 12 cL sans usage prévu » (R-GAS-4).
- Étant donné le seuil `SOON` du frigo à 1 jour et celui par défaut à 3 jours, alors un yaourt au frigo
  dont la DLC est dans 2 jours n'est pas `SOON`, alors qu'un paquet de pâtes au placard (sans seuil
  propre) dont la DDM est dans 2 jours l'est (R-GAS-2).
- Étant donné des courgettes `SOON` et de la crème sans usage prévu, alors un velouté qui utilise les deux
  est classé avant une quiche qui n'utilise que les courgettes, et deux appels donnent le même classement
  (R-GAS-20).
- Étant donné un chili au congélateur prévu pour le dîner de mardi, alors un rappel de décongélation est
  envoyé lundi à 20 h, une seule fois (R-GAS-32).
- Étant donné un mois où j'ai acheté pour 400 € et jeté pour 18 €, alors le taux de gaspillage est de
  4,5 % (R-GAS-42).
- Étant donné un inventaire qui constate 200 g de moins que le calcul, alors ces 200 g apparaissent en
  « pertes inexpliquées », pas dans le gaspillage (R-GAS-43).
