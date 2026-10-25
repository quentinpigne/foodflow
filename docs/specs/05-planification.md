# Spec 05 — Planification

Statut : **Validée** (2026-10-25)
Modèle : [Member, Preference, SlotSchedule, SlotRole, HabitualMeal, Meal, MealItem, PrepSession, GenerationRun](../modele-de-donnees.md#planification--spec-05)
Décisions : ADR-0004 (foyer, convives, préférences), ADR-0005 (validation), ADR-0011 (temps, batch
cooking), ADR-0012 (restes), ADR-0013 (génération déterministe), ADR-0014 (génération par IA),
ADR-0015 (ordonnancement)

## Concepts

```
Semaine du lundi 2 novembre                         objectif « Foyer » (version du 02/11)
───────────────────────────────────────────────────────────────────────────────────────────
Dim. 01  ⏲ Session de préparation 15:00–18:00 (3 h)  → chili ×2 (mar., jeu.), sauce bolognaise
Lun. 02  Petit-déj.  ☕ habituel « Tartines »                                    3,6 éq.
         Déjeuner    — hors foyer (cantine, travail)
         Dîner       Gratin de courgettes (sur place, 30 min) · yaourt          3,6 éq.
                     ↳ cuisiné pour 6 : reste prévu 2,4 portions → mer. déjeuner
Mar. 03  Dîner       Chili con carne (préparé dim., finition 10 min) · riz      3,6 éq.
Mer. 04  Déjeuner    Restes : gratin de courgettes (2,4 portions) · pomme       2,4 éq.
         Dîner       Lasagnes (sur place, 40 min ⚠ dépasse 30 min)  + 2 invités 5,6 éq.
…
Bilan prévu : énergie 1 980 kcal/j ✔ · fruits et légumes 4,2 portions/j ▼ · poisson 1 fois ▼ (incomplet : 1 repas vide)
```

- **Membre du foyer** (`Member`) et **préférences** (`Preference`) : ADR-0004.
- **Semaine type** : pour chaque jour et chaque créneau (`MealSlot`, spec 04), le temps actif
  disponible, l'heure à partir de laquelle on peut cuisiner et l'heure limite (`SlotSchedule`) ; la
  présence par défaut de chaque membre ; la **composition type** du créneau ; un éventuel **repas
  habituel**. Valeurs par défaut des temps : ADR-0011.
- **Composition type** (`SlotRole`) : les rôles d'un repas, dans l'ordre, chacun obligatoire ou
  facultatif, avec ce qui peut le remplir (catégories de recette, groupes nutritionnels).
  Par défaut : déjeuner et dîner = plat (obligatoire), accompagnement (facultatif), laitage ou fruit
  (facultatif) ; petit-déjeuner et goûter = repas habituel.
- **Repas habituel** (`HabitualMeal`) : composition fixe (« Tartines : 60 g de pain, 10 g de beurre,
  20 g de confiture, 250 mL de lait », par équivalent adulte), reprise par défaut sur un créneau
  certains jours.
- **Repas** (`Meal`) : une date, un créneau, des convives, des **éléments** (`MealItem`), un statut.
- **Élément de repas** : une recette (cuisinée sur place, préparée dans une session, ou issue d'un lot
  de préparation ou de restes), ou un aliment (yaourt, pomme, pain), avec sa quantité.
- **Session de préparation** (`PrepSession`) : ADR-0011.
- **Proposition de génération** (`GenerationRun`) : résultat d'une génération déterministe (ADR-0013) ou
  par IA (ADR-0014), en attente d'acceptation.

### Statuts d'un repas

| Statut | Signification | Stock, courses, bilan |
|---|---|---|
| `PLANNED` | Prévu | Compté dans les besoins et le bilan prévu |
| `DONE` | Cuisiné et mangé, validé | Mouvements de stock créés ; bilan réel |
| `SKIPPED` | N'a pas eu lieu | Rien |
| `OUT_OF_HOME` | Pris hors du foyer (restaurant, cantine pour tous) | Rien ; cible du jour réduite (R-NUT-31) |

## Règles

### Semaine type et présence

- **R-PLA-1** — La semaine type définit, pour chaque jour de la semaine et chaque créneau actif : le
  **temps actif disponible**, l'heure **à partir de laquelle** on peut cuisiner et l'**heure limite**
  (repas prêt). Le temps total disponible (actif + passif) = heure limite − heure de début.
- **R-PLA-2** — Chaque membre a une **présence par défaut** par jour et par créneau (absent le midi en
  semaine, par exemple).
- **R-PLA-3** — Un créneau peut être **hors foyer par défaut** certains jours ; ses repas sont créés au
  statut `OUT_OF_HOME`.
- **R-PLA-4** — La semaine commence le jour choisi dans les réglages (lundi par défaut).
- **R-PLA-5** — Créer un repas recopie les valeurs de la semaine type (temps, convives) ; elles se
  modifient ensuite **sur le repas** sans toucher à la semaine type.

### Repas, convives et éléments

- **R-PLA-10** — Il y a au plus un repas par date et par créneau.
- **R-PLA-11** — Les convives d'un repas sont ses membres présents et ses **invités** (coefficient
  saisi, 1,000 par défaut). Ses **équivalents adulte** = Σ des coefficients (ADR-0004). Un repas sans
  convive n'est ni cuisiné ni compté.
- **R-PLA-12** — Un élément de repas est l'un de :
  - une **recette cuisinée sur place**, pour un nombre de **portions cuisinées** (par défaut, les
    équivalents adulte du repas) ;
  - une **recette préparée à l'avance** dans une session de préparation (ADR-0011) ;
  - une **part d'un lot** de préparation ou de restes (en portions) ou d'un lot d'aliment de reste ;
  - un **aliment** (ou un produit), en quantité par équivalent adulte ou en quantité totale.
- **R-PLA-13** — Cuisiner **pour plus** que les équivalents adulte crée un **reste prévu**, qui doit être
  affecté à un repas ultérieur avant sa date limite (ADR-0012) ; sinon il est signalé.
- **R-PLA-14** — Chaque élément occupe un **rôle** de la composition type, ou aucun (ajout libre).
  Un rôle obligatoire vide est signalé, jamais bloquant.
- **R-PLA-15** — Un repas peut être **verrouillé** : la génération ne le modifie pas.

### Repas habituels

- **R-PLA-20** — Un repas habituel définit ses éléments **par équivalent adulte** ; il est mis à
  l'échelle des convives du repas.
- **R-PLA-21** — Un créneau peut avoir un repas habituel par défaut, certains jours : les repas de ce
  créneau sont créés avec ses éléments. On peut le remplacer ou le modifier pour un repas donné.
- **R-PLA-22** — Les repas habituels comptent dans les besoins, les courses, le stock (à la validation)
  et le bilan, comme les autres.

### Validation d'un repas (ADR-0005)

- **R-PLA-30** — Valider un repas propose, préremplies et ajustables :
  - les **sorties de stock** de chaque élément : ingrédients mis à l'échelle des portions cuisinées,
    pris dans les lots selon R-STK-36, avec la question de R-STK-37 pour un lot dont la date est dépassée ;
    pour une part de lot, la sortie de ce lot ;
  - les **restes** de chaque recette cuisinée (ADR-0012) : « rien » par défaut, ou les portions du reste
    prévu ;
  - les **convives** réels.
- **R-PLA-31** — La validation enregistre ce qui a été **réellement utilisé** et **mangé** (ingrédients et
  quantités, part mangée de chaque élément) : c'est la base du bilan réel (R-NUT-20), indépendante des
  modifications ultérieures des recettes (R-REC-3).
- **R-PLA-32** — Un ingrédient absent du stock (non suivi, oublié) est permis : il est enregistré comme
  utilisé sans sortie de stock, et signalé pour un éventuel inventaire.
- **R-PLA-33** — On peut **valider plusieurs repas en une fois** (une journée, les petits-déjeuners
  habituels de la semaine) avec les valeurs proposées.
- **R-PLA-34** — Une validation s'**annule** (R-STK-12) : le repas revient à `PLANNED`.
- **R-PLA-35** — Un repas dont la date est passée et qui n'est ni validé, ni sauté, ni hors foyer est
  **à valider** : il est signalé sur le tableau de bord.

### Sessions de préparation (ADR-0011)

- **R-PLA-40** — Une session de préparation est ajoutée à une date, avec une heure de début et un temps
  disponible. Elle contient des **préparations** : une recette, une quantité (portions ou quantité de
  rendement), un emplacement de rangement, et les repas qu'elle sert.
- **R-PLA-41** — La quantité d'une préparation = Σ des besoins des repas qu'elle sert (mis à l'échelle de
  leurs convives) ; elle peut être augmentée à la main.
- **R-PLA-42** — Une session a un **nombre de cuisiniers** (1 par défaut). Sa durée est celle de son
  **planning** (ADR-0015) : ordre des phases de ses préparations, en parallèle quand les cuisiniers et
  les équipements le permettent ; une préparation compte une fois, quelle que soit sa quantité
  (R-REC-15). Un dépassement du temps disponible est signalé.
- **R-PLA-45** — Le planning de la session s'affiche en **mode cuisine** : frise par cuisinier et par
  équipement, prochaine action à faire et minuteurs. Il est recalculé si l'on ajoute, retire ou modifie
  une préparation.
- **R-PLA-46** — Un repas cuisiné sur place a aussi un planning (ses éléments ensemble), calé pour finir à
  l'heure limite : il donne l'**heure de début** conseillée, et le repas est signalé si elle précède
  l'heure à partir de laquelle on peut cuisiner.
- **R-PLA-43** — Une préparation ne sert un repas que si sa date limite (date de la session +
  conservation au frigo, ou durée au congélateur si elle est rangée au congélateur) couvre la date du
  repas. Rangée au congélateur, elle demande une **décongélation** la veille du repas, rappelée sur le
  tableau de bord.
- **R-PLA-44** — Valider une session (ADR-0005) sort les ingrédients du stock et crée un **lot de
  préparation** par préparation (`PREPARED`, R-STK-10) ; les repas servis consomment ensuite ces lots.

### Signalements

- **R-PLA-50** — La planification manuelle n'est jamais bloquée ; ces situations sont **signalées** sur
  le repas, la journée ou la semaine :
  - temps actif ou heure limite dépassés ;
  - élément refusé par un convive présent ;
  - reste prévu non affecté, préparation ou reste utilisé après sa date limite ;
  - session de préparation trop longue ;
  - rôle obligatoire vide, créneau sans repas ;
  - repas à valider (R-PLA-35).

### Génération (ADR-0013, ADR-0014)

- **R-PLA-60** — Une génération porte sur une **période** et, au choix, sur tous les créneaux ou sur
  certains ; elle ne remplit que les repas **vides** ou **non verrouillés** que l'on désigne, et ne
  touche jamais un repas validé.
- **R-PLA-61** — Elle respecte les contraintes dures et le score de l'ADR-0013 ; les **réglages de
  génération** (poids, nombre d'itérations, plafond de coût) ont des valeurs par défaut modifiables.
- **R-PLA-62** — Les **restes** et les lots `SOON` sont placés en priorité (ADR-0012). Un lot dont la
  date est dépassée et qui n'a pas été jugé encore bon n'est jamais utilisé (R-STK-37).
- **R-PLA-63** — Si la période contient des sessions de préparation, la génération peut y affecter des
  recettes préparables à l'avance pour soulager les créneaux courts ; elle peut aussi proposer de
  **cuisiner pour plus** pour couvrir un créneau ultérieur.
- **R-PLA-64** — Le résultat est une **proposition** (`GenerationRun`) : on la consulte avec son bilan
  prévu, ses raisons et ses contraintes non satisfaites, puis on l'**accepte** en tout ou repas par repas.
  Rien n'est écrit avant l'acceptation.
- **R-PLA-65** — Une génération déterministe enregistre sa **graine** et ses entrées résumées : la
  relancer avec les mêmes données et la même graine redonne la même proposition. « Une autre
  proposition » change la graine.
- **R-PLA-66** — La génération par IA (ADR-0014) affiche les données envoyées et demande confirmation
  avant l'envoi ; ses recettes nouvelles deviennent des brouillons (R-REC-61), et les repas qui les
  utilisent ne sont créés qu'une fois les brouillons validés.

## Features

| ID | Priorité | Feature |
|---|---|---|
| PLA-01 | MVP | Membres du foyer : coefficient, présence par défaut |
| PLA-02 | MVP | Créneaux et semaine type : temps actif, heure de début, heure limite, hors foyer par défaut |
| PLA-03 | MVP | Calendrier des repas : saisie manuelle de recettes et d'aliments, convives et invités, statuts |
| PLA-04 | MVP | Validation d'un repas : sorties de stock, restes, consommation réelle ; validation groupée ; annulation |
| PLA-05 | MVP | Signalements (temps, restes, rôles, repas à valider) |
| PLA-06 | V1 | Préférences des membres (aliment, famille, recette) |
| PLA-07 | V1 | Composition type des créneaux, rôles |
| PLA-08 | V1 | Repas habituels |
| PLA-09 | V1 | Cuisiner pour plus et affecter les restes ; finir ou transformer un reste depuis le stock |
| PLA-10 | V1 | Sessions de préparation : préparations, temps, conservation, décongélation, validation |
| PLA-11 | V1 | Génération déterministe : proposition, raisons, acceptation partielle, autre proposition |
| PLA-12 | V1 | Génération par IA (option, désactivée par défaut) |
| PLA-13 | V1 | Copier une semaine passée comme point de départ |
| PLA-14 | V1 | Ordonnancement d'une session et d'un repas : planning, mode cuisine, heure de début conseillée (ADR-0015) |
| PLA-15 | V1 | Équipements de la cuisine et leur capacité |

### Critères d'acceptation

- Étant donné un dîner de semaine avec 30 min de temps actif, quand j'y place des lasagnes (40 min
  actives), alors le repas est créé et signalé « temps dépassé » (R-PLA-50).
- Étant donné un repas habituel « Tartines » de 60 g de pain par équivalent adulte, quand il est repris un
  jour à 3,6 équivalents adulte, alors il demande 216 g de pain (R-PLA-20).
- Étant donné un gratin cuisiné pour 6 portions au dîner de lundi pour 3,6 équivalents adulte, alors un
  reste prévu de 2,4 portions est signalé tant qu'il n'est pas affecté à un repas avant sa date limite
  (R-PLA-13).
- Étant donné une session de préparation le dimanche avec un chili qui se conserve 3 jours au frigo,
  quand je l'affecte au dîner du jeudi suivant (4 jours), alors c'est refusé sauf s'il est rangé au
  congélateur, et la décongélation est rappelée le mercredi (R-PLA-43).
- Étant donné une semaine avec deux dîners verrouillés, quand je lance une génération, alors ces deux
  dîners sont inchangés dans la proposition (R-PLA-15, R-PLA-60).
- Étant donné une proposition générée avec la graine 42, quand je relance la génération avec les mêmes
  données et la même graine, alors j'obtiens la même proposition (R-PLA-65).
- Étant donné qu'un convive refuse les champignons, alors aucune proposition générée ne contient de
  recette avec des champignons pour un repas où il est présent (ADR-0013).
- Étant donné un repas validé, quand j'annule sa validation, alors ses sorties de stock et ses restes
  sont annulés et il revient à `PLANNED` (R-PLA-34).
