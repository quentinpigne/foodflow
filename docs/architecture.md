# Architecture cible

Statut : **Validée** (2026-10-25)

Même architecture que Cashflow (ADR-0009) : monolithe en couches autour d'un **cœur de calcul pur**,
une base SQLite à fichier unique, une API REST décrite en OpenAPI consommée par une SPA Angular servie
par le même binaire. Ce document décrit ce qui est propre à Foodflow ; pour le reste, il renvoie à
`../cashflow/docs/architecture.md`.

## Vue d'ensemble

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ api : routes REST (axum, DTO, OpenAPI) · sous-commandes CLI                   │
│       (serve, migrate, demo, ciqual-import, export, backup, set-password)     │
└───────────────┬──────────────────────────────────────────────────────────────┘
                │ appelle
┌───────────────▼──────────────────────────────────────────────────────────────┐
│ services : un par cas d'usage                                                 │
│  RecordLot · ValidateMeal · ValidatePrepSession · StartShoppingTrip           │
│  FinishShoppingTrip · ImportRecipeFromUrl · CommitRecipeDraft · GenerateMenu  │
│  AcceptProposal · RunInventory · ReconcileTrip · ExportAll …                  │
│  → une transaction SQLite par service : charger, calculer, écrire             │
└───────┬────────────────────────────────────────────────┬─────────────────────┘
        │ calcule avec                                    │ lit, écrit, appelle
┌───────▼──────────────────────────────────┐   ┌──────────▼───────────────────┐
│ domain (pur, sans I/O, sans horloge)      │   │ db : requêtes sqlx (SQLite)  │
│  shared : Amount, Measure, Money, Ratio,  │   ├──────────────────────────────┤
│           Ids, Date, erreurs              │   │ readers : table Ciqual (XML),│
│  foods : conversions, résolution des      │   │  JSON-LD schema.org          │
│          réglages hérités, nutriments     │   ├──────────────────────────────┤
│  stock : quantité d'un lot, péremption,   │   │ clients (traits) :           │
│          premier périmé premier sorti     │   │  Open Food Facts, pages de   │
│  recipes : mise à l'échelle, analyse des  │   │  recettes, IA, Cashflow,     │
│          lignes, rattachement, valeurs,   │   │  Web Push                    │
│          sous-recettes, cycles            │   ├──────────────────────────────┤
│  nutrition : versions, critères, bilan    │   │ jobs : sauvegardes, alertes, │
│  planning : ordonnancement (ADR-0015),    │   │  push, rechargement du       │
│          génération (ADR-0013), contrôle  │   │  certificat TLS              │
│          des propositions IA (ADR-0014),  │   ├──────────────────────────────┤
│          propositions de validation       │   │ files : images, sauvegardes, │
│  shopping : besoins, couverture datée,    │   │  certificats, clés VAPID     │
│          liste, arrondi, prix             │   └──────────────────────────────┘
│  waste : priorités, lots sans usage,      │
│          suggestions, statistiques        │
│  cashflow : projection, plafond,          │
│          classement des candidates        │
│  reporting : encadrés de l'accueil        │
└───────────────────────────────────────────┘
```

Le domaine ne connaît ni la base, ni le réseau, ni l'horloge : la date du jour (et, pour les
notifications, l'heure dans le fuseau du foyer) lui est passée en paramètre. Les **traits** sont
réservés aux clients de services extérieurs, pour les remplacer par des doublures en test : aucun test
n'appelle le réseau.

## Modules du domaine

| Module | Contenu | Spec |
|---|---|---|
| `shared` | `Amount` (mg, µL, millièmes de pièce ou de portion), `Measure`, `Money`, `Ratio`, identifiants, dates, erreurs | ADR-0001 |
| `foods` | Familles, aliments, produits, unités ; conversions ; réglages hérités ; valeurs nutritionnelles connues, estimées, inconnues | 01 |
| `stock` | Lots, mouvements, quantité calculée, date limite effective, états de péremption, sélection des lots | 02 |
| `recipes` | Recettes, ingrédients, sous-recettes, mise à l'échelle, analyse et rattachement des lignes importées, valeurs et coût calculés | 03 |
| `nutrition` | Versions d'objectif, critères, groupes et portions, apports, bilan | 04 |
| `planning` | Repas, convives, composition, repas habituels, sessions de préparation, ordonnancement, génération, contrôle des propositions | 05 |
| `shopping` | Besoins, couverture datée, liste, ajustements, arrondi au conditionnement, prix de référence | 06 |
| `waste` | Liste prioritaire, lots sans usage prévu, suggestions de recettes, statistiques | 07 |
| `cashflow` | Projection de fin de mois, plafond proposé, classement des transactions candidates | 08 |
| `reporting` | Encadrés de l'accueil | 09 |

Dépendances autorisées : `shared` ← tous ; `foods` ← tous sauf `shared` ; `stock` ← `recipes`,
`planning`, `shopping`, `waste` ; `recipes` ← `nutrition`, `planning`, `shopping`, `waste` ;
`nutrition` ← `planning` ; `planning` ← `shopping`, `waste`, `reporting`. Pas de cycle.

## Flux clés

### Validation d'un repas (spec 05, ADR-0005)

```
ProposeMealValidation(meal)            [service : charge repas, recettes, lots, réglages]
  → planning::validation::propose      [domain : sorties mises à l'échelle, lots par date limite,
                                         questions R-STK-37, restes prévus]
  ← proposition affichée, ajustée par l'utilisateur
ValidateMeal(meal, décisions)          → 1 transaction SQLite : mouvements COOKED, LEFTOVER, WASTE,
                                         CHECKED ; consommation réelle ; statut DONE
```

### Liste de courses (spec 06)

`shopping::needs` construit les besoins datés à partir des repas, sessions de préparation, repas
habituels et stocks minimums ; `shopping::coverage` simule les sorties dans l'ordre chronologique sur
une copie du stock ; `shopping::list` regroupe, arrondit au conditionnement et applique les ajustements.
La même simulation sert à `waste::unused_lots` (R-GAS-4) : un seul moteur, deux lectures.

### Génération et ordonnancement (ADR-0013, ADR-0015)

Fonctions pures, déterministes, bornées en itérations. Le service charge un instantané des données
(recettes, stock, objectif, préférences, semaine type), appelle le domaine, et enregistre la
proposition (`GenerationRun`). La génération par IA appelle le client IA, puis passe sa réponse au même
contrôle (`planning::ai_check`) avant de l'enregistrer.

### Mode magasin hors connexion (spec 06, ADR-0009)

`StartShoppingTrip` fige la liste. L'interface la garde dans IndexedDB ; le service worker sert
l'application hors ligne. Chaque geste est stocké localement avec son horodatage, puis envoyé par lot
à `POST /shopping-trips/{id}/events` au retour du réseau ; le serveur applique « le dernier geste
l'emporte » par champ (R-COU-42).

## Gestion des erreurs

Comme Cashflow : erreurs métier typées du domaine, qui citent la règle `R-*` violée ; l'interface les
traduit. Une réponse invalide d'un service extérieur (Open Food Facts, IA, Cashflow, page de recette) est
une **donnée** (brouillon incomplet, élément écarté et signalé), jamais une panne de l'application.

## Stratégie de test

| Niveau | Cible | Outils |
|---|---|---|
| Unitaire | Moteurs du domaine, règle par règle (`R-*`) | `cargo test` ; Vitest pour `quantity.ts`, `money.ts` |
| Propriétés | Quantité d'un lot = inventaire + Σ mouvements ; annulation = état initial ; conversions aller-retour ; génération sans contrainte dure violée et reproductible ; planning sans surcharge d'équipement | `proptest` |
| Golden files | Analyse des lignes d'ingrédients ; extraction JSON-LD de pages enregistrées ; extrait de la table Ciqual ; réponses enregistrées de l'IA et d'Open Food Facts | `insta` |
| Intégration | Services avec une SQLite en mémoire, clients extérieurs remplacés par des doublures | — |
| Aller-retour | Export JSON → import → export identique ; recette → JSON-LD → recette | — |
| Bout en bout | Planifier une semaine, faire les courses (hors ligne compris), valider un repas avec restes | Playwright |

Les exemples chiffrés des critères d'acceptation des specs sont repris **tels quels** comme tests.

## Sécurité et confidentialité

- Aucune donnée personnelle ni aucun secret dans le dépôt : base, images, certificats, clés VAPID,
  jetons (IA, Cashflow, fournisseur DNS) sont hors du dépôt et hors de l'export.
- HTTPS dès la V1 (ADR-0009), mot de passe unique et session par cookie `Secure` (comme Cashflow).
- Flux sortants, tous déclenchés par une action ou un réglage explicite (ADR-0007) : Open Food Facts,
  pages de recettes, IA, services de notification push, Cashflow (réseau local). Chacun a un délai
  d'expiration court et une taille de réponse bornée ; la page d'une recette est téléchargée sans
  exécuter de script.
- Journalisation : jamais le contenu d'une requête IA, d'un jeton ou d'un abonnement push au niveau
  INFO.

## Migrations et données de référence

- Migrations versionnées, en avant seulement, immuables une fois publiées ; sauvegarde avant chaque
  migration (comme Cashflow, ADR-0017 de Cashflow).
- Données initiales par un mécanisme de seed **distinct** et idempotent : nutriments, unités générales,
  groupes nutritionnels et tailles de portion, rattachement des groupes Ciqual, sélection des aliments
  courants, rayons, créneaux et semaine type par défaut, modèle « Repères PNNS 2019 ».
- La table Ciqual elle-même n'est **pas** dans le dépôt : `foodflow ciqual-import <fichier>` l'importe
  (R-ALI-50). Un extrait de quelques aliments, sous licence ouverte Etalab avec attribution, sert de
  fixture de test.

## Projection sur la stack

Stack, conventions Rust, lints, SQLite, CI et livraison : **identiques à Cashflow** (ADR-0009 ; cf.
`../cashflow/docs/architecture.md`, § « Projection sur la stack »). Ce qui diffère :

### Organisation du dépôt

```
foodflow/
├── Cargo.toml                 ← workspace Rust
├── crates/
│   ├── domain/                ← cœur de calcul pur
│   │   └── src/{shared,foods,stock,recipes,nutrition,planning,
│   │            shopping,waste,cashflow,reporting}.rs + dossiers homonymes
│   └── server/                ← binaire `foodflow`
│       ├── src/api.rs + api/
│       ├── src/services.rs + services/
│       ├── src/db.rs + db/
│       ├── src/readers.rs + readers/    ← Ciqual XML, JSON-LD
│       ├── src/clients.rs + clients/    ← Open Food Facts, pages web, IA, Cashflow, Web Push
│       ├── src/jobs.rs + jobs/          ← tâches de fond
│       ├── migrations/
│       └── seed/
├── web/                       ← SPA Angular + service worker
│   └── src/app/api/           ← client généré
├── openapi.json · .sqlx/ · fixtures/ · deploy/ · docs/
├── README.md · CLAUDE.md
```

### Dépendances propres à Foodflow

- `domain` : mêmes dépendances autorisées que Cashflow (`rust_decimal`, `chrono`, `uuid`, `serde`,
  `thiserror`, `utoipa` en option) ; ni `reqwest`, ni analyseur HTML, ni horloge. Les flottants y sont
  interdits.
- `server` : en plus de la base Cashflow, un client HTTP (`reqwest`, TLS `rustls`), un analyseur HTML
  pour extraire le JSON-LD, un lecteur XML pour Ciqual, le chiffrement Web Push (RFC 8291) avec clés
  VAPID, le service HTTPS par `rustls` avec rechargement du certificat. Les crates précis sont choisis à
  la phase concernée et consignés ici.

### Interface (`web/`)

- **Écrans implémentés directement**, sans maquette préalable, en reprenant la structure de Cashflow
  (menu latéral, Angular Material, encadrés de l'accueil, formulaires typés) ; le propriétaire les
  corrige visuellement. Il n'y a pas de document d'écrans : la référence est l'application.
- Pages : Accueil, Planning (semaine, repas, sessions de préparation), Recettes (liste, fiche, mode
  cuisine, import), Stock (emplacements, lots, à consommer en priorité, inventaire), Courses (sessions,
  liste, mode magasin), Bilan (nutrition, gaspillage), Réglages (foyer et membres, semaine type,
  objectif, aliments et familles, rayons et magasins, équipements, notifications, Cashflow, IA).
- **Téléphone d'abord** pour l'accueil, le mode cuisine, le mode magasin, la validation d'un repas et le
  stock ; la planification de la semaine, la saisie de recettes et les réglages sont pensés pour
  l'ordinateur et restent utilisables sur téléphone.
- Quantités : module `quantity.ts` (analyse d'une saisie « 1,5 kg », « 2 c. à s. », formatage), et
  `money.ts` repris de Cashflow ; aucune arithmétique flottante sur une quantité ou un montant.
- PWA (`@angular/service-worker`) : installation, mode magasin hors connexion, notifications push.
  Scan de code-barres par la caméra (API `BarcodeDetector`, bibliothèque de repli sinon). Mode cuisine :
  écran maintenu allumé (API Wake Lock).

### Commandes

Celles de Cashflow, avec le binaire `foodflow` (`cargo run -p foodflow-server -- …`), plus :

| Tâche | Commande |
|---|---|
| Importer la table Ciqual | `cargo run -p foodflow-server -- ciqual-import <fichier.xml>` |
| Base de démonstration | `cargo run -p foodflow-server -- demo` (aliments, recettes, une semaine planifiée, un stock ; mot de passe `demo-foodflow`) |

### Déploiement

Comme Cashflow (ADR-0018 de Cashflow) : image Docker `linux/amd64`, déployée sur ubuntu01 par
`deploy/deploy.sh`, volume `/data` (`foodflow.db`, `images/`, `backups/`, `certs/`, `vapid/`). En plus :
renouvellement du certificat Let's Encrypt par un outil ACME planifié sur ubuntu01 (challenge DNS-01),
qui écrit dans `certs/` ; le binaire recharge le certificat quand le fichier change (ADR-0009).
