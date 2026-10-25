#!/bin/sh
# Serveur des tests de bout en bout : base neuve, jeu de démonstration, date figée.
set -eu
cd "$(dirname "$0")/../.."
DATA=target/e2e-data
TODAY=2026-10-25
rm -rf "$DATA"
cargo build -q -p foodflow-server
# Pages de recettes fictives servies par le serveur lui-même : l'import les télécharge sans sortir sur Internet.
mkdir -p web/dist/foodflow/browser/e2e-pages
cp fixtures/recipes/*.html web/dist/foodflow/browser/e2e-pages/
./target/debug/foodflow --data-dir "$DATA" demo --today "$TODAY"
exec ./target/debug/foodflow --data-dir "$DATA" serve --addr 127.0.0.1:3199 --web-dir web/dist/foodflow/browser --today "$TODAY"
