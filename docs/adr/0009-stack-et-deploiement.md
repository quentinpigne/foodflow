# ADR-0009 — Stack et déploiement : ceux de Cashflow, plus une PWA

- **Statut** : Accepté (2026-10-25)
- **Date** : 2026-10-24
- **Concerne** : architecture, toutes les specs

## Contexte

Cashflow a validé et mis en œuvre une stack (ADR-0009, 0013, 0015 à 0018 de Cashflow) :

- backend Rust : workspace `crates/{domain,server}`, axum, sqlx + SQLite sans ORM, rust_decimal, chrono,
  uuid v7 ; cœur métier pur, couches `api` → `services` → `db` ;
- frontend Angular (standalone, signals, sans zone.js), Angular Material, Reactive Forms typés, ECharts ;
  client d'API généré depuis l'OpenAPI ;
- un binaire unique qui sert l'API et la SPA, image Docker `linux/amd64` déployée sur ubuntu01,
  développement sur le Mac sans Docker, CI GitHub Actions ;
- mot de passe unique (argon2id), session par cookie, réseau local, **HTTP simple en V1**.

Reprendre cette stack, c'est partager le savoir-faire, les conventions et une partie du code.
Foodflow ajoute un besoin : **la liste de courses doit fonctionner en magasin sans réseau**, ce qui
demande une PWA avec un service worker. Or **les navigateurs n'autorisent un service worker qu'en HTTPS**
(ou sur `localhost`). Le magasin est par ailleurs hors du réseau local : la liste doit avoir été chargée
avant de partir.

## Options envisagées

### Stack
Reprendre à l'identique celle de Cashflow — décidé par le propriétaire le 2026-10-24.

### HTTPS pour la PWA
1. **HTTP en V1, sans mode hors connexion** — ✅ identique à Cashflow / ❌ la liste n'est consultable
   en magasin que si l'onglet est resté ouvert ; aucune garantie.
2. **HTTPS dès la V1, servi par axum (rustls)** avec un certificat d'une **autorité interne** installée
   sur le téléphone et l'ordinateur — ✅ PWA complète, sans domaine / ❌ installer un certificat racine
   sur chaque appareil.
3. **HTTPS dès la V1 avec un domaine personnel** et un certificat Let's Encrypt obtenu par challenge DNS
   (le serveur reste injoignable depuis Internet) — ✅ aucun certificat à installer, renouvellement
   automatique / ❌ demande un domaine et un accès à l'API de son DNS.

Les options 2 et 3 tranchent aussi la question Q15 de Cashflow : le même mécanisme pourra servir aux
deux applications.

## Décision

- **Stack identique à Cashflow**, avec les mêmes règles de code et les mêmes vérifications avant livraison.
  Binaire `foodflow`, volume `/data` avec `foodflow.db`.
- **PWA** (`@angular/service-worker`) pour l'installation sur le téléphone et le **mode magasin hors
  connexion** : la liste de courses en cours est gardée sur l'appareil, les articles cochés hors ligne
  sont envoyés au retour du réseau.
- **HTTPS dès la V1, option 3** (décidée le 2026-10-25) : un sous-domaine du domaine personnel
  (`foodflow.<domaine>`) avec un certificat Let's Encrypt obtenu par **challenge DNS-01**.
  - Le certificat est obtenu et renouvelé **hors du binaire**, par un outil ACME (lego ou certbot avec le
    plugin du fournisseur DNS) planifié sur ubuntu01, qui écrit le certificat dans le volume.
    Le binaire le lit au démarrage et le recharge quand le fichier change, sans redémarrage.
  - axum sert le HTTPS lui-même (`rustls`), sans reverse proxy, comme le prévoyait Cashflow (ADR-0015).
  - Le nom résout vers l'adresse locale du serveur : enregistrement DNS public pointant vers une adresse
    privée, ou DNS local. Le serveur reste injoignable depuis Internet ; seul le fournisseur DNS est
    contacté, pour le challenge.
  - Cookie de session `Secure`, en plus de `HttpOnly` et `SameSite=Strict`.
- Les conventions de modèle de Cashflow sont reprises : identifiants UUIDv7, dates calendaires sans
  fuseau, horodatages UTC réservés à l'audit (ADR-0011 et ADR-0012 de Cashflow).

## Conséquences

- Le même mécanisme de certificat pourra servir à Cashflow (Q15 de Cashflow) : un sous-domaine de plus.
- Certains routeurs bloquent les réponses DNS publiques contenant une adresse privée (protection contre
  le « DNS rebinding ») : si c'est le cas, une entrée DNS locale (routeur, `/etc/hosts`) est nécessaire.
- Le jeton d'API du fournisseur DNS est un secret du serveur, hors du dépôt et hors de l'image.

- La synchronisation hors connexion est limitée à la liste de courses (cocher, ajouter un article) :
  le reste de l'application exige le réseau local.
- Un article coché hors ligne puis modifié ailleurs se résout par une règle simple, décrite dans la spec
  `COU` (le dernier geste l'emporte, horodaté par l'appareil).
- Les deux applications tournent sur ubuntu01, chacune dans son conteneur et sur son port.
