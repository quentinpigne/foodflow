//! Erreurs métier typées. Chacune porte un code stable et, si elle existe, la règle violée.

use super::{Amount, Date};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DomainError {
    // ----- Types partagés
    #[error("quantité hors limites")]
    QuantityOutOfRange,
    #[error("montant hors limites")]
    AmountOutOfRange,

    // ----- Aliments (spec 01)
    #[error("une famille ne peut pas être sa propre ancêtre")]
    FoodGroupCycle,
    #[error("une famille qui contient des aliments, des sous-familles ou des produits ne peut pas être supprimée")]
    FoodGroupInUse,
    #[error("un aliment de ce nom existe déjà")]
    FoodNameTaken,
    #[error("cet alias est déjà utilisé")]
    AliasTaken,
    #[error("une famille de ce nom existe déjà à cet endroit")]
    FoodGroupNameTaken,
    #[error("code-barres invalide")]
    InvalidBarcode,
    #[error("ce code-barres désigne déjà un autre produit")]
    BarcodeTaken,
    #[error("un produit est rattaché à un aliment ou à une famille, pas aux deux")]
    ProductAttachment,
    #[error("les valeurs nutritionnelles importées de Ciqual ne se modifient pas directement")]
    CiqualValueLocked,
    #[error("conversion impossible : il manque la densité de l'aliment")]
    MissingDensity,
    #[error("conversion impossible : il manque le poids d'une pièce de l'aliment")]
    MissingPieceWeight,
    #[error("une valeur positive est attendue")]
    NotPositive,
    #[error("un aliment utilisé ne peut pas être supprimé")]
    FoodInUse,

    // ----- Stock (spec 02)
    #[error("un emplacement de ce nom existe déjà")]
    LocationNameTaken,
    #[error("un emplacement qui contient des lots ne peut pas être supprimé")]
    LocationInUse,
    #[error("un lot contient exactement un produit, un aliment ou une recette")]
    LotContent,
    #[error("une date de péremption est requise sauf pour un produit sans date")]
    ExpiryDateRequired,
    #[error("un mouvement ne peut pas être daté dans le futur")]
    MovementInFuture,
    #[error("un mouvement ne peut pas précéder le dernier inventaire du lot ({0})")]
    MovementBeforeInventory(Date),
    #[error("ce mouvement est déjà annulé")]
    AlreadyReversed,
    #[error("une annulation ne s'annule pas")]
    ReverseOfReversal,
    #[error("la quantité du mouvement ne correspond pas au mode de suivi du lot")]
    TrackingModeMismatch,
    #[error("la quantité d'un lot ne peut pas être négative")]
    NegativeQuantity,
    #[error("le lot est déjà à cet emplacement")]
    SameLocation,
    #[error("on ne congèle pas un lot après sa date limite")]
    FreezeAfterExpiry,
    #[error("un lot décongelé ne se recongèle pas")]
    RefreezeThawed,
    #[error("on ne déplace pas plus que la quantité du lot ({0})")]
    MoveExceedsLot(Amount),
    #[error("la nouvelle date limite doit suivre la date du jugement")]
    CheckedUntilBeforeDate,
    #[error("un lot épuisé ne bouge plus")]
    LotDepleted,

    // ----- Recettes (spec 03)
    #[error("une recette a au moins un ingrédient")]
    RecipeWithoutIngredient,
    #[error("rendement invalide : un nombre de portions positif, ou une quantité positive avec sa mesure")]
    InvalidYield,
    #[error("un ingrédient vise un aliment, une famille ou une sous-recette, et un seul")]
    IngredientTarget,
    #[error("une recette ne peut pas être, directement ou non, son propre ingrédient")]
    RecipeCycle,
    #[error("un ingrédient à volonté n'a pas de quantité")]
    ToTasteWithQuantity,
    #[error("une recette congelable indique sa durée au congélateur")]
    FrozenDaysRequired,

    // ----- Objectifs nutritionnels (spec 04)
    #[error("une version d'objectif existe déjà à cette date")]
    GoalVersionTaken,
    #[error("une version déjà entrée en vigueur ne se modifie pas : en créer une nouvelle")]
    GoalVersionLocked,
    #[error("critère invalide : {0}")]
    InvalidCriterion(&'static str),
    #[error("les parts d'énergie des créneaux actifs totalisent 100 %")]
    EnergySharesNotFull,

    // ----- Planification (spec 05)
    #[error("un membre du foyer porte déjà ce nom")]
    MemberNameTaken,
    #[error("l'heure à partir de laquelle on cuisine précède l'heure limite")]
    InvalidSchedule,
    #[error("il y a déjà un repas sur ce créneau ce jour-là")]
    MealSlotTaken,
    #[error("un élément de repas est une recette cuisinée, une part d'un lot ou un aliment")]
    MealItemContent,
    #[error("un repas validé ne se modifie pas : annuler d'abord sa validation")]
    MealValidated,
    #[error("seul un repas prévu se valide")]
    MealNotPlanned,
    #[error("un repas habituel de ce nom existe déjà")]
    HabitualMealNameTaken,

    // ----- IA (ADR-0014)
    #[error("réponse de l'IA invalide : {0}")]
    AiResponse(&'static str),
}

impl DomainError {
    /// Code stable exposé par l'API (`application/problem+json`, champ `code`).
    pub fn code(&self) -> &'static str {
        use DomainError::*;
        match self {
            QuantityOutOfRange => "QUANTITY_OUT_OF_RANGE",
            AmountOutOfRange => "AMOUNT_OUT_OF_RANGE",
            FoodGroupCycle => "FOOD_GROUP_CYCLE",
            FoodGroupInUse => "FOOD_GROUP_IN_USE",
            FoodNameTaken => "FOOD_NAME_TAKEN",
            AliasTaken => "ALIAS_TAKEN",
            FoodGroupNameTaken => "FOOD_GROUP_NAME_TAKEN",
            InvalidBarcode => "INVALID_BARCODE",
            BarcodeTaken => "BARCODE_TAKEN",
            ProductAttachment => "PRODUCT_ATTACHMENT",
            CiqualValueLocked => "CIQUAL_VALUE_LOCKED",
            MissingDensity => "MISSING_DENSITY",
            MissingPieceWeight => "MISSING_PIECE_WEIGHT",
            NotPositive => "NOT_POSITIVE",
            FoodInUse => "FOOD_IN_USE",
            LocationNameTaken => "LOCATION_NAME_TAKEN",
            LocationInUse => "LOCATION_IN_USE",
            LotContent => "LOT_CONTENT",
            ExpiryDateRequired => "EXPIRY_DATE_REQUIRED",
            MovementInFuture => "MOVEMENT_IN_FUTURE",
            MovementBeforeInventory(_) => "MOVEMENT_BEFORE_INVENTORY",
            AlreadyReversed => "ALREADY_REVERSED",
            ReverseOfReversal => "REVERSE_OF_REVERSAL",
            TrackingModeMismatch => "TRACKING_MODE_MISMATCH",
            NegativeQuantity => "NEGATIVE_QUANTITY",
            SameLocation => "SAME_LOCATION",
            FreezeAfterExpiry => "FREEZE_AFTER_EXPIRY",
            RefreezeThawed => "REFREEZE_THAWED",
            MoveExceedsLot(_) => "MOVE_EXCEEDS_LOT",
            CheckedUntilBeforeDate => "CHECKED_UNTIL_BEFORE_DATE",
            LotDepleted => "LOT_DEPLETED",
            RecipeWithoutIngredient => "RECIPE_WITHOUT_INGREDIENT",
            InvalidYield => "INVALID_YIELD",
            IngredientTarget => "INGREDIENT_TARGET",
            RecipeCycle => "RECIPE_CYCLE",
            ToTasteWithQuantity => "TO_TASTE_WITH_QUANTITY",
            FrozenDaysRequired => "FROZEN_DAYS_REQUIRED",
            GoalVersionTaken => "GOAL_VERSION_TAKEN",
            GoalVersionLocked => "GOAL_VERSION_LOCKED",
            InvalidCriterion(_) => "INVALID_CRITERION",
            EnergySharesNotFull => "ENERGY_SHARES_NOT_FULL",
            MemberNameTaken => "MEMBER_NAME_TAKEN",
            InvalidSchedule => "INVALID_SCHEDULE",
            MealSlotTaken => "MEAL_SLOT_TAKEN",
            MealItemContent => "MEAL_ITEM_CONTENT",
            MealValidated => "MEAL_VALIDATED",
            MealNotPlanned => "MEAL_NOT_PLANNED",
            HabitualMealNameTaken => "HABITUAL_MEAL_NAME_TAKEN",
            AiResponse(_) => "AI_RESPONSE",
        }
    }

    /// Règle métier violée, si l'erreur en fait respecter une.
    pub fn rule(&self) -> Option<&'static str> {
        use DomainError::*;
        Some(match self {
            QuantityOutOfRange | AmountOutOfRange | NotPositive | SameLocation | LotDepleted => return None,
            FoodGroupCycle => "R-ALI-1",
            FoodGroupInUse => "R-ALI-3",
            FoodNameTaken | AliasTaken => "R-ALI-10",
            FoodGroupNameTaken => "R-ALI-1",
            CiqualValueLocked => "R-ALI-11",
            MissingDensity | MissingPieceWeight => "R-ALI-31",
            ProductAttachment => "R-ALI-40",
            InvalidBarcode | BarcodeTaken => "R-ALI-41",
            FoodInUse => "R-ALI-13",
            LocationNameTaken | LocationInUse => return None,
            LotContent => "R-STK-1",
            ExpiryDateRequired => "R-STK-30",
            NegativeQuantity => "R-STK-3",
            AlreadyReversed | ReverseOfReversal => "R-STK-11",
            MovementInFuture | MovementBeforeInventory(_) => "R-STK-14",
            TrackingModeMismatch => "R-STK-20",
            FreezeAfterExpiry => "R-STK-32",
            RefreezeThawed => "R-STK-33",
            MoveExceedsLot(_) => "R-STK-15",
            CheckedUntilBeforeDate => "R-STK-37",
            RecipeWithoutIngredient | InvalidYield => "R-REC-1",
            IngredientTarget => "R-REC-20",
            ToTasteWithQuantity => "R-REC-22",
            RecipeCycle => "R-REC-30",
            FrozenDaysRequired => "R-REC-42",
            GoalVersionTaken => "R-NUT-2",
            GoalVersionLocked => "R-NUT-3",
            InvalidCriterion(_) => "R-NUT-10",
            EnergySharesNotFull => "R-NUT-30",
            MemberNameTaken | HabitualMealNameTaken => return None,
            InvalidSchedule => "R-PLA-1",
            MealSlotTaken => "R-PLA-10",
            MealItemContent => "R-PLA-12",
            MealValidated => "R-PLA-31",
            MealNotPlanned => "R-PLA-30",
            AiResponse(_) => "R-REC-82",
        })
    }
}
