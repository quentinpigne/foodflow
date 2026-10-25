//! Signalements de la planification (R-PLA-50) : la saisie manuelle n'est jamais bloquée, les situations
//! à revoir sont signalées sur le repas.

use serde::Serialize;

use crate::shared::ids::MemberId;
use crate::shared::{Amount, Date, Ratio};

use super::MealStatus;

/// Une situation à revoir sur un repas.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Signal {
    /// Temps actif des recettes cuisinées sur place supérieur au temps actif disponible.
    TimeExceeded { needed: u32, available: u32 },
    /// Temps total (actif et passif) supérieur au temps entre l'heure de début et l'heure limite.
    ReadyByExceeded { needed: u32, available: u32 },
    /// Un convive présent refuse un élément.
    Refused { member: MemberId, item: String },
    /// Un rôle obligatoire de la composition type est vide (R-PLA-14).
    RequiredRoleEmpty { role: String },
    /// Reste prévu non affecté à un repas ultérieur, en millièmes de portion (R-PLA-13).
    LeftoverUnassigned { portions: Amount },
    /// Reste prévu affecté à un repas après sa date limite.
    LeftoverPastDeadline { deadline: Date },
    /// Repas passé, ni validé, ni sauté, ni hors foyer (R-PLA-35).
    ToValidate,
    /// Repas prévu sans convive : ni cuisiné ni compté (R-PLA-11).
    NoDiner,
}

/// Ce qu'il faut savoir d'un repas pour le signaler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MealFacts {
    pub date: Date,
    pub status: MealStatus,
    pub adult_equivalents: Ratio,
    pub available_active_minutes: u32,
    /// Heure limite − heure de début (R-PLA-1).
    pub window_minutes: u32,
    /// Temps actif des recettes cuisinées sur place, chacune comptée une fois (R-REC-15).
    pub active_minutes: u32,
    /// Temps total des recettes cuisinées sur place (actif et passif) ; elles sont menées de front, la plus
    /// longue fixe la durée.
    pub total_minutes: u32,
    pub empty_required_roles: Vec<String>,
    /// Convives présents qui refusent un élément, avec le nom de l'élément.
    pub refusals: Vec<(MemberId, String)>,
}

/// Signalements d'un repas, à la date du jour (R-PLA-50).
pub fn meal_signals(facts: &MealFacts, today: Date) -> Vec<Signal> {
    let mut out = Vec::new();
    if facts.status != MealStatus::Planned {
        return out;
    }
    if facts.date < today {
        out.push(Signal::ToValidate);
    }
    if facts.adult_equivalents.thousandths() <= 0 {
        out.push(Signal::NoDiner);
    }
    if facts.active_minutes > facts.available_active_minutes {
        out.push(Signal::TimeExceeded { needed: facts.active_minutes, available: facts.available_active_minutes });
    }
    if facts.total_minutes > facts.window_minutes {
        out.push(Signal::ReadyByExceeded { needed: facts.total_minutes, available: facts.window_minutes });
    }
    out.extend(facts.empty_required_roles.iter().map(|role| Signal::RequiredRoleEmpty { role: role.clone() }));
    out.extend(facts.refusals.iter().map(|(member, item)| Signal::Refused { member: *member, item: item.clone() }));
    out
}

/// Signalements d'un reste prévu : portions non affectées, ou affectées après la date limite (R-PLA-13,
/// ADR-0012). `uses` : date et portions de chaque repas qui le mange.
pub fn leftover_signals(planned: Amount, deadline: Date, uses: &[(Date, Amount)]) -> Vec<Signal> {
    let mut out = Vec::new();
    let assigned: i64 = uses.iter().map(|(_, portions)| portions.raw()).sum();
    if planned.raw() > assigned {
        out.push(Signal::LeftoverUnassigned { portions: Amount::from_raw(planned.raw() - assigned) });
    }
    if uses.iter().any(|(date, _)| *date > deadline) {
        out.push(Signal::LeftoverPastDeadline { deadline });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn dinner() -> MealFacts {
        MealFacts {
            date: d("2026-11-04"),
            status: MealStatus::Planned,
            adult_equivalents: Ratio::from_thousandths(3_600),
            available_active_minutes: 30,
            window_minutes: 60,
            active_minutes: 20,
            total_minutes: 50,
            empty_required_roles: vec![],
            refusals: vec![],
        }
    }

    /// Critère d'acceptation : des lasagnes (40 min actives) sur un dîner de semaine à 30 min sont acceptées
    /// et signalées « temps dépassé » (R-PLA-50).
    #[test]
    fn r_pla_50_time_exceeded_is_a_signal() {
        let lasagna = MealFacts { active_minutes: 40, total_minutes: 75, ..dinner() };
        assert_eq!(
            meal_signals(&lasagna, d("2026-11-01")),
            vec![
                Signal::TimeExceeded { needed: 40, available: 30 },
                Signal::ReadyByExceeded { needed: 75, available: 60 },
            ]
        );
        assert!(meal_signals(&dinner(), d("2026-11-01")).is_empty());
    }

    /// Un repas passé non validé est à valider ; un repas validé ou sauté n'est plus signalé (R-PLA-35).
    #[test]
    fn r_pla_35_to_validate() {
        assert_eq!(meal_signals(&dinner(), d("2026-11-05")), vec![Signal::ToValidate]);
        let done = MealFacts { status: MealStatus::Done, ..dinner() };
        assert!(meal_signals(&done, d("2026-11-05")).is_empty());
    }

    #[test]
    fn r_pla_14_and_r_pla_11_roles_and_diners() {
        let empty = MealFacts {
            adult_equivalents: Ratio::from_thousandths(0),
            empty_required_roles: vec!["Plat".into()],
            ..dinner()
        };
        assert_eq!(
            meal_signals(&empty, d("2026-11-01")),
            vec![Signal::NoDiner, Signal::RequiredRoleEmpty { role: "Plat".into() }]
        );
    }

    /// Critère d'acceptation : le reste prévu de 2,4 portions est signalé tant qu'il n'est pas affecté avant sa
    /// date limite (R-PLA-13).
    #[test]
    fn r_pla_13_leftover_signals() {
        let planned = Amount::from_raw(2_400);
        let deadline = d("2026-11-04");
        assert_eq!(leftover_signals(planned, deadline, &[]), vec![Signal::LeftoverUnassigned { portions: planned }]);
        assert!(leftover_signals(planned, deadline, &[(d("2026-11-04"), planned)]).is_empty());
        assert_eq!(
            leftover_signals(planned, deadline, &[(d("2026-11-06"), planned)]),
            vec![Signal::LeftoverPastDeadline { deadline }]
        );
    }
}
