# Pages de recettes de test

Pages HTML **fictives**, écrites pour les tests du lecteur JSON-LD et de l'import par URL
(`crates/server/src/readers/jsonld.rs`, `services/recipe_import.rs`). Elles imitent les deux formes
courantes : une recette dans un `@graph` (avec sections d'étapes), et une recette seule.

`gateau-sans-de.html` imite un site qui omet le « de » après un contenant et écrit « unité » pour une pièce
(« 1 sachet levure », « 3 unité pomme »), avec une accroche dans le nom et des mots-clés « Recette … ».
