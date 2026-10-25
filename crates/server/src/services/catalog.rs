//! Référentiel chargé en mémoire : familles, aliments et produits, avec la résolution des réglages hérités
//! (R-ALI-2). Quelques milliers de lignes au plus : on le charge à chaque cas d'usage qui en a besoin.

use std::collections::HashMap;

use foodflow_domain::foods::groups::{ancestors, resolve};
use foodflow_domain::foods::{NutritionGroup, TrackingMode};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, ProductId};
use foodflow_domain::stock::expiry::ShelfLives;
use foodflow_domain::{Amount, Measure};
use serde::Serialize;
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use crate::db;
use crate::db::foods::FoodRow;
use crate::db::groups::{GroupRow, Settings};
use crate::db::products::ProductRow;

/// Réglages effectifs, après héritage (R-ALI-2). Sans valeur dans toute la chaîne : quantité, en masse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct EffectiveSettings {
    pub nutrition_group: Option<NutritionGroup>,
    pub stock_measure: Measure,
    pub tracking_mode: TrackingMode,
    pub shelf_life_days: Option<u32>,
    pub opened_shelf_life_days: Option<u32>,
    pub frozen_shelf_life_days: Option<u32>,
    pub thawed_shelf_life_days: Option<u32>,
    pub portion_measure: Option<Measure>,
    pub portion_amount: Option<Amount>,
}

impl EffectiveSettings {
    pub fn shelf(&self) -> ShelfLives {
        ShelfLives {
            unopened_days: self.shelf_life_days,
            opened_days: self.opened_shelf_life_days,
            frozen_days: self.frozen_shelf_life_days,
            thawed_days: self.thawed_shelf_life_days,
        }
    }
}

fn effective(chain: &[&Settings]) -> EffectiveSettings {
    let portion = resolve(chain.iter().map(|s| s.portion_measure.zip(s.portion_amount)));
    EffectiveSettings {
        nutrition_group: resolve(chain.iter().map(|s| s.nutrition_group)),
        stock_measure: resolve(chain.iter().map(|s| s.stock_measure)).unwrap_or(Measure::Mass),
        tracking_mode: resolve(chain.iter().map(|s| s.tracking_mode)).unwrap_or(TrackingMode::Quantity),
        shelf_life_days: resolve(chain.iter().map(|s| s.shelf_life_days)),
        opened_shelf_life_days: resolve(chain.iter().map(|s| s.opened_shelf_life_days)),
        frozen_shelf_life_days: resolve(chain.iter().map(|s| s.frozen_shelf_life_days)),
        thawed_shelf_life_days: resolve(chain.iter().map(|s| s.thawed_shelf_life_days)),
        portion_measure: portion.map(|(m, _)| m),
        portion_amount: portion.map(|(_, a)| a),
    }
}

pub struct Catalog {
    pub groups: HashMap<FoodGroupId, GroupRow>,
    pub parents: HashMap<FoodGroupId, Option<FoodGroupId>>,
    pub foods: HashMap<FoodId, FoodRow>,
    pub products: HashMap<ProductId, ProductRow>,
}

impl Catalog {
    pub async fn load(conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        let groups: HashMap<_, _> = db::groups::list(&mut *conn).await?.into_iter().map(|g| (g.id, g)).collect();
        let parents = groups.values().map(|g| (g.id, g.parent_id)).collect();
        let foods = db::foods::list(&mut *conn).await?.into_iter().map(|f| (f.id, f)).collect();
        let products = db::products::list(&mut *conn).await?.into_iter().map(|p| (p.id, p)).collect();
        Ok(Self { groups, parents, foods, products })
    }

    /// Réglages de la famille, puis de ses ancêtres.
    fn group_chain(&self, group: FoodGroupId) -> Vec<&Settings> {
        std::iter::once(group)
            .chain(ancestors(&self.parents, group))
            .filter_map(|id| self.groups.get(&id))
            .map(|g| &g.settings)
            .collect()
    }

    pub fn group_settings(&self, group: FoodGroupId) -> EffectiveSettings {
        effective(&self.group_chain(group))
    }

    pub fn food_settings(&self, food: &FoodRow) -> EffectiveSettings {
        let mut chain = vec![&food.settings];
        chain.extend(self.group_chain(food.group_id));
        effective(&chain)
    }

    /// Réglages qui s'appliquent à un produit : ceux de son aliment, ou de sa famille ; sa mesure propre
    /// prime pour un produit rattaché à une famille (modèle de données).
    pub fn product_settings(&self, product: &ProductRow) -> EffectiveSettings {
        let mut settings = match (product.food_id.and_then(|f| self.foods.get(&f)), product.group_id) {
            (Some(food), _) => self.food_settings(food),
            (None, Some(group)) => self.group_settings(group),
            (None, None) => effective(&[]),
        };
        if let (None, Some(measure)) = (product.food_id, product.stock_measure) {
            settings.stock_measure = measure;
        }
        settings
    }

    /// Nom de la famille et de ses ancêtres, de la racine à la famille (« Produits laitiers › Laits »).
    pub fn group_path(&self, group: FoodGroupId) -> String {
        let mut names: Vec<&str> = std::iter::once(group)
            .chain(ancestors(&self.parents, group))
            .filter_map(|id| self.groups.get(&id))
            .map(|g| g.name.as_str())
            .collect();
        names.reverse();
        names.join(" › ")
    }
}
