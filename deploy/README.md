# Installation et déploiement

Foodflow tourne comme Cashflow dans **un seul environnement**, sur le serveur personnel (ADR-0009) : une
image Docker, un dossier de l'hôte monté sur `/data` qui contient toutes les données, sur le réseau local.
Le HTTPS par Let's Encrypt viendra quand le mécanisme de certificat sera tranché (Q16) ; d'ici là,
l'application est servie en HTTP : la caméra du téléphone (scan), l'installation et le mode magasin hors
connexion demandent le HTTPS, le lecteur de code-barres fonctionne sans.

L'image est **construite par la CI** et publiée sur `ghcr.io/quentinpigne/foodflow`, étiquetée par son commit
(et `latest`), seulement pour un push sur `main` dont tous les contrôles sont passés. Le serveur ne construit
rien : il télécharge l'image.

## Prérequis sur le serveur

- Docker avec le plugin compose, et Git.
- L'accès à l'image : rien à faire si le paquet `foodflow` est public sur GitHub ; s'il est privé,
  `docker login ghcr.io -u <utilisateur>` une fois, avec un jeton (classic) limité à `read:packages` (le
  même que pour Cashflow convient).
- Le **disque chiffré**, comme pour Cashflow.
- Le port 3100 accessible depuis le réseau local seulement (pas de redirection sur la box).

## Premier démarrage

Les données vivent dans un dossier de l'hôte, désigné par `FOODFLOW_DATA_PATH` dans `deploy/.env` (non
versionné). Le conteneur tourne avec l'uid et le gid **1000** (le premier utilisateur d'Ubuntu) : le dossier
doit lui appartenir, sinon l'application ne peut pas y écrire. Pour un autre utilisateur, ajouter
`FOODFLOW_UID` et `FOODFLOW_GID` (donnés par `id -u` et `id -g`) dans `deploy/.env`.

```sh
git clone <dépôt> foodflow && cd foodflow
mkdir -p ~/foodflow-data                                     # sur le disque chiffré
echo "FOODFLOW_DATA_PATH=$HOME/foodflow-data" > deploy/.env
deploy/deploy.sh                                             # télécharge l'image et lance l'application
docker compose -f deploy/compose.yaml exec foodflow foodflow set-password
```

L'application répond sur `http://<adresse-du-serveur>:3100`. Les données initiales (nutriments, unités,
emplacements) sont chargées au premier démarrage.

## Importer la table Ciqual

La table n'est pas dans le dépôt (R-ALI-50). Télécharger l'archive XML sur https://ciqual.anses.fr
(« Table Ciqual au format XML »), la décompresser dans le dossier de données, puis :

```sh
. deploy/.env
mkdir -p "$FOODFLOW_DATA_PATH/ciqual" && cp <dossier des XML>/*.xml "$FOODFLOW_DATA_PATH/ciqual/"
docker compose -f deploy/compose.yaml exec foodflow foodflow ciqual-import /data/ciqual
```

L'import est idempotent : le relancer avec une table plus récente met à jour les aliments sans toucher à ce
qui a été modifié à la main (R-ALI-11, R-ALI-51).

## Mettre à jour

Une fois la CI du commit terminée et verte (sinon son image n'existe pas, et le script s'arrête) :

```sh
deploy/deploy.sh
```

Le script récupère la version (`git pull`), télécharge l'image du commit
(`ghcr.io/quentinpigne/foodflow:<commit>`), **sauvegarde la base**, puis relance. Les migrations de schéma
s'appliquent au démarrage, après une sauvegarde automatique supplémentaire. Le journal
`deploy/deployments.log` garde la liste des versions déployées.

## Revenir en arrière

1. Relancer la version précédente, dont l'image reste sur le registre :

   ```sh
   tail deploy/deployments.log          # repérer le commit précédent
   deploy/deploy.sh <commit-précédent>
   ```

2. **Si la version annulée contenait une migration**, restaurer aussi la sauvegarde prise juste avant le
   déploiement (`foodflow-<horodatage>-pre-deploy.db`) :

   ```sh
   docker compose -f deploy/compose.yaml stop
   docker compose -f deploy/compose.yaml run --rm --entrypoint sh foodflow -c \
     'ls /data/backups && cp /data/backups/<fichier> /data/foodflow.db && rm -f /data/foodflow.db-wal /data/foodflow.db-shm'
   docker compose -f deploy/compose.yaml start
   ```

## Sauvegardes

- **Automatiques**, dans le dossier de données (`$FOODFLOW_DATA_PATH/backups`, `/data/backups` dans le
  conteneur) : une par jour, une avant chaque migration et une avant chaque déploiement.
- **Manuelle** : `docker compose -f deploy/compose.yaml exec foodflow foodflow backup`.
- **Hors du serveur** : à organiser côté serveur, comme pour Cashflow, par exemple une copie quotidienne des
  sauvegardes de la base et des photos des recettes, qui n'existent qu'en un exemplaire :

  ```sh
  . deploy/.env
  cp -n "$FOODFLOW_DATA_PATH"/backups/*.db /mnt/sauvegardes/foodflow/
  mkdir -p /mnt/sauvegardes/foodflow/images && cp -Rn "$FOODFLOW_DATA_PATH"/images/. /mnt/sauvegardes/foodflow/images/
  ```

- **Export ouvert** (ADR-0008) : Réglages › Données › Exporter, ou
  `docker compose -f deploy/compose.yaml exec foodflow foodflow export > export.json`.

## Commandes utiles

| Besoin | Commande |
|---|---|
| Journaux | `docker compose -f deploy/compose.yaml logs -f` |
| Changer le mot de passe | `docker compose -f deploy/compose.yaml exec foodflow foodflow set-password` |
| Instance d'essai jetable (données fictives, port 3101) | `docker run --rm -p 3101:3100 --entrypoint sh ghcr.io/quentinpigne/foodflow -c 'foodflow demo && foodflow serve'` (sans le compose, pour ne pas toucher au dossier de données) |
