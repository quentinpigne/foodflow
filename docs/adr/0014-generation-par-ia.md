# ADR-0014 — Génération de menus par IA

- **Statut** : Accepté (2026-10-25), révisé le 2026-10-27
- **Date** : 2026-10-25
- **Concerne** : `PLA`, `REC`, ADR-0005, ADR-0007, ADR-0013

## Contexte

La génération déterministe (ADR-0013) compose avec les recettes connues et des critères chiffrés. Une IA
peut apporter ce qu'elle ne sait pas faire : comprendre une consigne libre (« semaine italienne », « plus
léger après les fêtes »), et proposer des recettes nouvelles. Elle est optionnelle et désactivée par
défaut (ADR-0007). Décision du propriétaire (2026-10-25) : l'IA compose surtout avec les recettes
existantes, et peut proposer quelques recettes nouvelles, soumises à revue.

## Options envisagées

1. **Choisir seulement** parmi les recettes existantes — ✅ prévisible / ❌ pas de découverte.
2. **Choisir et proposer**, les recettes nouvelles arrivant en brouillon — ✅ découverte sous contrôle /
   ❌ un brouillon de plus à relire.
3. **Libre**, recettes créées directement — ❌ contraire à ADR-0005.

## Décision

Option 2.

### Requête
- Même périmètre que la génération déterministe (ADR-0013), plus une **consigne libre** facultative.
- Données envoyées, **affichées avant l'envoi** et limitées au nécessaire (ADR-0007) :
  - créneaux à remplir avec leur temps disponible, leur composition type et leurs convives (membres
    **désignés par une lettre**, avec leur coefficient et leurs préférences ; jamais leur nom) ;
  - recettes candidates (identifiant, nom, catégorie, temps, conservation, valeurs par portion,
    groupes nutritionnels principaux), déjà filtrées par les contraintes dures ;
  - lots à utiliser en priorité (restes, lots `SOON`) ;
  - critères de la version d'objectif en vigueur, plafond de coût éventuel.
- La réponse est demandée dans un **format JSON imposé** : pour chaque créneau, des éléments qui
  référencent une recette ou un aliment **par identifiant**, ou une **recette nouvelle** (nom, rendement,
  temps, lignes d'ingrédients, étapes), avec une justification courte.
- Fournisseur : **toute API compatible OpenAI** (`POST /v1/chat/completions`), probablement un
  proxy LiteLLM devant le modèle choisi (Q12, 2026-10-25). Adresse, clé, nom du modèle et plafond
  d'appels mensuel sont dans la configuration du serveur ; changer de modèle ne demande aucun code.
  Le format JSON est demandé par schéma (`response_format` de type `json_schema`) quand le modèle le
  permet, et la réponse est de toute façon validée par le domaine. Le nombre de recettes nouvelles par
  demande est plafonné (3 par défaut).

### Contrôle de la réponse
- Le domaine **vérifie** la réponse : identifiants existants, contraintes dures (ADR-0013). Un élément
  invalide est écarté et signalé, jamais corrigé en silence.
- Chaque recette nouvelle devient un **brouillon de recette** (R-REC-61), dont les lignes d'ingrédients
  passent par la même analyse et le même rattachement qu'un import par URL (R-REC-63 à R-REC-66).
- La proposition est présentée **comme une proposition déterministe** : bilan prévu, score, raisons,
  contraintes non satisfaites. Les repas qui utilisent une recette nouvelle restent en attente tant que
  son brouillon n'est pas validé.

## Conséquences

- La génération par IA ne contourne aucune règle : elle propose, le domaine contrôle, l'utilisateur valide.
- Le format d'échange est versionné et testé avec des réponses enregistrées (pas d'appel réseau en test).
- Coût maîtrisé : appel seulement à la demande, plafond mensuel, envoi limité aux recettes candidates.
- Hors connexion à Internet ou sans clé, l'option est grisée ; la génération déterministe reste
  disponible.

## Révision du 2026-10-27

Décisions du propriétaire, qui remplacent les points correspondants ci-dessus.

### Configuration dans les réglages
- L'IA s'active dans les **Réglages** (désactivée par défaut, ADR-0007). On y choisit le **fournisseur** —
  **Mistral** ou **OpenAI**, chacun avec son adresse par défaut, que l'on peut remplacer pour tout service
  compatible (proxy LiteLLM, autre hébergeur) —, le **modèle** et la **clé d'accès**.
- La clé est gardée en base, mais **jamais réaffichée** (on peut seulement la remplacer ou l'effacer),
  jamais journalisée et **exclue de l'export** (ADR-0008) ; elle reste dans les sauvegardes de la base,
  sur le serveur. Les deux fournisseurs exposent la même API (`POST {adresse}/chat/completions`) : un seul
  client, sans code propre à un fournisseur.
- Réglages associés : **plafond d'appels par mois** (50 par défaut) et **nombre de recettes nouvelles par
  planning** (3 par défaut, jusqu'à une par repas).

### Création de recettes par l'IA (REC-19)
Hors planning, l'IA peut créer **une recette**, à partir de :
- une **consigne libre** (« un plat végétarien rapide ») ;
- le **stock à écouler** (lots `SOON`, restes, à date dépassée jugés encore bons) ;
- une **recette existante**, dont elle fait une variante (« plus légère », « sans gluten ») ; l'originale
  ne change pas ;
- **son propre choix**, d'après le stock et les préférences du foyer (membres désignés par une lettre).

Les données envoyées sont affichées avant l'envoi, comme pour un planning. La recette arrive en **brouillon**
(R-REC-61), relu comme un import : ses lignes d'ingrédients sont analysées et rattachées (R-REC-63 à
R-REC-66).
