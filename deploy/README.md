# Installation et déploiement

Foodflow tourne comme Cashflow dans **un seul environnement**, sur le serveur personnel (ADR-0009) : une
image Docker, un volume qui contient toutes les données, sur le réseau local. Le HTTPS par Let's Encrypt
viendra quand le mécanisme de certificat sera tranché (Q16) ; d'ici là, l'application est servie en HTTP,
et la PWA (mode magasin hors connexion) n'est pas encore disponible.

## Prérequis sur le serveur

- Docker avec le plugin compose, et Git.
- Le disque chiffré, comme pour Cashflow.
- Le port 3100 accessible depuis le réseau local seulement.

## Premier démarrage

```sh
git clone <dépôt> foodflow && cd foodflow
deploy/deploy.sh                                             # construit l'image et lance l'application
docker compose -f deploy/compose.yaml exec foodflow foodflow set-password
```

L'application répond sur `http://<adresse-du-serveur>:3100`. Les données initiales (nutriments, unités,
emplacements) sont chargées au premier démarrage.

## Importer la table Ciqual

La table n'est pas dans le dépôt (R-ALI-50). Télécharger l'archive XML sur https://ciqual.anses.fr
(« Table Ciqual au format XML »), la décompresser sur le serveur, puis :

```sh
docker compose -f deploy/compose.yaml cp <dossier des XML> foodflow:/data/ciqual
docker compose -f deploy/compose.yaml exec foodflow foodflow ciqual-import /data/ciqual
```

L'import est idempotent : le relancer avec une table plus récente met à jour les aliments sans toucher à ce
qui a été modifié à la main (R-ALI-11, R-ALI-51).

## Mettre à jour, revenir en arrière, sauvegardes

Comme Cashflow (`../cashflow/deploy/README.md`), en remplaçant `cashflow` par `foodflow` :

- `deploy/deploy.sh` : récupère la version, sauvegarde la base, construit l'image `foodflow:<commit>` et
  relance ; `deploy/deploy.sh <commit>` revient à une version déjà construite.
- Sauvegardes automatiques dans `/data/backups` (quotidienne, avant migration, avant déploiement).
- Export complet : Réglages › Données › Exporter, ou
  `docker compose -f deploy/compose.yaml exec foodflow foodflow export > export.json`.
