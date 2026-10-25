//! Ordonnancement des phases de cuisson (ADR-0015) : planning d'un ensemble de recettes, avec leurs
//! sous-recettes, pour un nombre de cuisiniers et des équipements de capacité donnée.
//!
//! Ordonnancement de liste, déterministe : à chaque instant, les phases prêtes démarrent par priorité
//! (plus long chemin restant, puis ordre d'entrée), tant que cuisiniers et équipements le permettent.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Nature d'une phase : active (occupe un cuisinier) ou passive (cuisson, repos).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PhaseKind {
    Active,
    Passive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    pub kind: PhaseKind,
    pub minutes: u32,
    /// Équipements occupés pendant toute la phase (indices dans `capacities`).
    pub equipment: Vec<usize>,
}

/// Une tâche : les phases d'une recette, et les tâches qui doivent finir avant elle (ses sous-recettes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub phases: Vec<Phase>,
    pub after: Vec<usize>,
}

impl Task {
    /// Sans phases décrites : une phase active puis une phase passive, sans équipement (R-REC-40).
    pub fn simple(active: u32, passive: u32, after: Vec<usize>) -> Self {
        let phases = [(PhaseKind::Active, active), (PhaseKind::Passive, passive)]
            .into_iter()
            .filter(|(_, m)| *m > 0)
            .map(|(kind, minutes)| Phase { kind, minutes, equipment: vec![] })
            .collect();
        Self { phases, after }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Slot {
    pub task: usize,
    pub phase: usize,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Schedule {
    pub slots: Vec<Slot>,
    /// Durée totale, en minutes.
    pub makespan: u32,
}

/// Plus long chemin restant de chaque tâche : sa durée plus celle de la plus longue chaîne qui l'attend.
fn tails(tasks: &[Task]) -> Vec<u32> {
    let mut memo: HashMap<usize, u32> = HashMap::new();
    fn tail(i: usize, tasks: &[Task], memo: &mut HashMap<usize, u32>, depth: usize) -> u32 {
        if let Some(v) = memo.get(&i) {
            return *v;
        }
        let own: u32 = tasks[i].phases.iter().map(|p| p.minutes).sum();
        let next = if depth > tasks.len() {
            0
        } else {
            (0..tasks.len())
                .filter(|j| tasks[*j].after.contains(&i))
                .map(|j| tail(j, tasks, memo, depth + 1))
                .max()
                .unwrap_or(0)
        };
        memo.insert(i, own + next);
        own + next
    }
    (0..tasks.len()).map(|i| tail(i, tasks, &mut memo, 0)).collect()
}

/// Calcule le planning (ADR-0015). Les dépendances doivent être sans cycle (R-REC-30).
pub fn schedule(tasks: &[Task], cooks: u32, capacities: &[u32]) -> Schedule {
    let cooks = cooks.max(1);
    let priority = tails(tasks);
    let mut next_phase = vec![0usize; tasks.len()];
    let mut ready_at = vec![0u32; tasks.len()];
    let mut finished_at: Vec<Option<u32>> = tasks.iter().map(|t| t.phases.is_empty().then_some(0)).collect();
    let mut running: Vec<Slot> = Vec::new();
    let mut slots = Vec::new();
    let mut now = 0u32;
    loop {
        // Libère ce qui se termine maintenant.
        running.retain(|s| s.end > now);
        // Phases prêtes, par priorité.
        let mut ready: Vec<usize> = (0..tasks.len())
            .filter(|&i| finished_at[i].is_none() && running.iter().all(|s| s.task != i))
            .filter(|&i| ready_at[i] <= now)
            .filter(|&i| {
                tasks[i].after.iter().all(|d| finished_at.get(*d).copied().flatten().is_some_and(|f| f <= now))
            })
            .collect();
        ready.sort_by_key(|&i| (std::cmp::Reverse(priority[i]), i));
        for i in ready {
            let phase = &tasks[i].phases[next_phase[i]];
            let busy_cooks =
                running.iter().filter(|s| tasks[s.task].phases[s.phase].kind == PhaseKind::Active).count() as u32;
            if phase.kind == PhaseKind::Active && busy_cooks >= cooks {
                continue;
            }
            let equipment_free = phase.equipment.iter().all(|e| {
                let used =
                    running.iter().filter(|s| tasks[s.task].phases[s.phase].equipment.contains(e)).count() as u32;
                used < capacities.get(*e).copied().unwrap_or(u32::MAX)
            });
            if !equipment_free {
                continue;
            }
            let slot = Slot { task: i, phase: next_phase[i], start: now, end: now + phase.minutes };
            running.push(slot);
            slots.push(slot);
            next_phase[i] += 1;
            ready_at[i] = slot.end;
            if next_phase[i] == tasks[i].phases.len() {
                finished_at[i] = Some(slot.end);
            }
        }
        if finished_at.iter().all(Option::is_some) {
            break;
        }
        // Prochain événement : la fin d'une phase en cours, ou une tâche bloquée qui devient prête.
        let next = running.iter().map(|s| s.end).filter(|e| *e > now).min();
        match next {
            Some(t) => now = t,
            // Rien ne tourne et rien ne peut démarrer : dépendance impossible (cycle) ; on arrête.
            None => break,
        }
    }
    let makespan = slots.iter().map(|s| s.end).max().unwrap_or(0);
    Schedule { slots, makespan }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Critère d'acceptation de la spec 03 : sauce (20 + 90) et gratin (15 + 45), un cuisinier → 1 h 50.
    #[test]
    fn adr_0015_passive_phases_overlap() {
        let tasks = [Task::simple(20, 90, vec![]), Task::simple(15, 45, vec![])];
        let plan = schedule(&tasks, 1, &[]);
        assert_eq!(plan.makespan, 110);
        assert_eq!(plan.slots[0], Slot { task: 0, phase: 0, start: 0, end: 20 });
    }

    #[test]
    fn sub_recipe_finishes_first() {
        // Lasagnes (40 + 50) après leur sauce (20 + 90) : 20 + 90 + 40 + 50.
        let tasks = [Task::simple(20, 90, vec![]), Task::simple(40, 50, vec![0])];
        assert_eq!(schedule(&tasks, 1, &[]).makespan, 200);
    }

    #[test]
    fn oven_capacity_and_cooks() {
        let oven = |m| Phase { kind: PhaseKind::Passive, minutes: m, equipment: vec![0] };
        let active = |m| Phase { kind: PhaseKind::Active, minutes: m, equipment: vec![] };
        let a = Task { phases: vec![active(10), oven(30)], after: vec![] };
        let b = Task { phases: vec![active(10), oven(30)], after: vec![] };
        // Un four, un cuisinier : 10, 10, puis le second plat attend le four (10 + 30 + 30).
        assert_eq!(schedule(&[a.clone(), b.clone()], 1, &[1]).makespan, 70);
        // Deux places au four : 10 + 10 + 30.
        assert_eq!(schedule(&[a.clone(), b.clone()], 1, &[2]).makespan, 50);
        // Deux cuisiniers, deux places : 10 + 30.
        assert_eq!(schedule(&[a, b], 2, &[2]).makespan, 40);
    }

    #[test]
    fn deterministic_and_empty() {
        let tasks = [Task::simple(5, 0, vec![]), Task::simple(5, 0, vec![])];
        assert_eq!(schedule(&tasks, 1, &[]), schedule(&tasks, 1, &[]));
        assert_eq!(schedule(&[], 1, &[]).makespan, 0);
        assert_eq!(schedule(&[Task::simple(0, 0, vec![])], 1, &[]).makespan, 0);
    }
}
