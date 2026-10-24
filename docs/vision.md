# Vision produit

Statut : **Brouillon** (2026-10-24)

## Le problème

Nourrir le foyer demande de jongler entre plusieurs questions, que rien ne relie aujourd'hui :

- **Qu'est-ce qu'on a ?** Le contenu du garde-manger, du frigo et du congélateur, et ce qui va périmer.
- **Qu'est-ce qu'on mange cette semaine ?** Des repas variés et équilibrés, qui respectent un objectif
  de calories, et qui utilisent ce qu'on a déjà.
- **Qu'est-ce qu'il faut acheter ?** La liste de ce qui manque pour les repas prévus, rien de plus.
- **Combien ça coûte ?** Le coût d'un repas, d'une semaine, et sa place dans le budget alimentation
  suivi dans Cashflow.

## Pour qui

Le foyer, piloté par un seul utilisateur (ADR-0006) depuis l'ordinateur et le téléphone (en cuisine et
en magasin).

Les repas sont planifiés **pour le foyer**, avec un objectif nutritionnel global (ADR-0004) :

- chaque membre du foyer compte pour un **équivalent adulte** (un jeune enfant : 0,6) et a ses **goûts**
  (aime, n'aime pas, refuse), que la planification respecte ;
- un repas peut compter **des invités** ou des absents, exceptionnellement : les quantités et les courses
  suivent.

Les profils nutritionnels individuels, les allergies et régimes, et le journal alimentaire par personne
viendront en V2, sur ces mêmes membres.

Application personnelle, auto-hébergée sur le même serveur que Cashflow, avec la même stack (ADR-0009).

## Objectifs

1. **Savoir ce qu'on a** : un stock fiable, suivi par produit au niveau de détail choisi (quantité exacte
   ou simple présence), avec emplacement et date de péremption.
2. **Planifier des repas équilibrés** : un calendrier de repas, rempli à la main, généré
   automatiquement selon un objectif (algorithme déterministe), ou proposé par une IA (optionnel).
   L'équilibre se mesure en calories, en macronutriments, en groupes d'aliments (repères PNNS) et en
   variété. Ses critères sont réglables et changent dans le temps. Le menu tient compte du **temps
   disponible** à chaque repas et peut s'organiser autour d'une **session de préparation**
   (batch cooking, le dimanche par exemple) où l'on cuisine à l'avance des recettes ou des éléments de
   recettes pour la semaine (ADR-0011).
3. **Acheter juste ce qui manque** : une liste de courses calculée à partir des repas prévus moins le
   stock, utilisable en magasin sur le téléphone, même sans réseau.
4. **Gaspiller moins** : alerter avant péremption, proposer des recettes qui utilisent ce qui va
   périmer, remettre les restes en stock.
5. **Maîtriser le coût** : relever les prix aux courses, estimer le coût des recettes et des semaines,
   et relier les dépenses alimentaires aux budgets de Cashflow.

## Indicateurs de succès (à affiner)

- Planifier une semaine et obtenir la liste de courses prend **moins de 10 minutes**.
- Après validation des courses puis des repas cuisinés, le stock affiché correspond au stock réel
  pour les produits suivis en quantité.
- Une recette importée depuis une URL est utilisable (ingrédients rattachés aux aliments) après une
  revue de **moins de 2 minutes**.
- Une même demande de génération, avec le même stock, les mêmes objectifs et les mêmes recettes, donne
  **toujours le même menu**.

## Principes

- **Rien ne bouge sans validation.** Les courses entrent en stock et les repas cuisinés en sortent
  seulement après une validation explicite, avec des quantités ajustables. Un import de recette passe
  par une revue avant d'être enregistré.
- **Paramétrable plutôt que figé.** Mode de suivi du stock par produit, critères d'équilibre,
  structure des repas de la journée : tout est réglable et peut évoluer.
- **Des chiffres honnêtes.** Une valeur nutritionnelle ou un coût estimé indique sa source et sa
  complétude (« 3 ingrédients sans valeur nutritionnelle »). Un chiffre incomplet est signalé, jamais
  présenté comme exact.
- **Auto-hébergé, services tiers optionnels.** Ciqual est importée en local. Open Food Facts et l'IA
  sont des appels explicites et désactivables. L'application reste pleinement utilisable sans eux.
- **Mes données sont exportables** dans un format ouvert.

## Périmètre (proposition)

| Domaine | Inclus | Code |
|---|---|---|
| Aliments | Référentiel : aliments génériques (Ciqual), produits du commerce (Open Food Facts, code-barres), saisie manuelle ; valeurs nutritionnelles ; unités et conversions | `ALI` |
| Stock | Emplacements, lots, quantités ou présence selon le produit, péremption, entrées et sorties validées, restes | `STK` |
| Recettes | Saisie, import depuis une URL (schema.org/Recipe) avec revue, portions, étapes, sous-recettes, temps actif et passif, préparation à l'avance, valeurs nutritionnelles et coût calculés | `REC` |
| Objectifs nutritionnels | Calories, macronutriments, groupes d'aliments, variété ; versionnés dans le temps | `NUT` |
| Planification | Semaine type (créneaux, temps disponible, sessions de préparation), calendrier des repas, saisie manuelle, génération déterministe, génération par IA (optionnelle), batch cooking, bilan de la semaine | `PLA` |
| Courses | Liste calculée (besoins − stock), rayons, mode magasin hors connexion, validation vers le stock, prix | `COU` |
| Anti-gaspillage | Alertes de péremption, recettes « à finir », suivi de ce qui est jeté | `GAS` |
| Lien avec Cashflow | Budgets alimentation, rapprochement des courses avec les transactions | `CSH` |
| Tableau de bord | Ce qui périme, repas du jour, bilan nutritionnel, coût et budget | `TDB` |

## Reporté en V2

- **Profils nutritionnels individuels** : objectif propre à chaque membre.
- **Allergies, régimes et contraintes médicales** : contraintes strictes, distinctes des goûts (ADR-0004).
- **Journal alimentaire par personne** : ce que chacun a réellement mangé.

## Non-objectifs

- **Multi-utilisateur.** Un seul compte, qui saisit pour le foyer (ADR-0006).
- **Conseil médical ou diététique.** Les repères (PNNS, apports de référence) sont indicatifs, même en V2.
- **Commande en ligne et drive.** La liste de courses ne s'envoie pas à une enseigne.
- **Import de recettes par photo ou OCR.**
- **Comparateur de prix entre enseignes.**
