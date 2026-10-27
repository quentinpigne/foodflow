//! Clients de services extérieurs (ADR-0007). Chacun est un trait, pour être remplacé par une doublure dans
//! les tests : aucun test n'appelle le réseau.

pub mod ai;
pub mod photos;
pub mod web;
