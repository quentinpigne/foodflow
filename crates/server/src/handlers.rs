//! Handlers HTTP : extraient la requête, appellent un service et mettent en forme la réponse. Aucun
//! calcul ni aucune requête SQL ici. Chacun est décrit pour le contrat OpenAPI (utoipa).

pub mod auth;
pub mod dashboard;
pub mod foods;
pub mod groups;
pub mod locations;
pub mod products;
pub mod settings;
pub mod stock;
pub mod system;
pub mod units;
