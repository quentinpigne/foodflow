/** Critères d'objectif : unité d'affichage et libellé (spec 04). */

import { Criterion } from '../api/models/criterion';
import { CRITERION_KIND_LABELS, NUTRITION_GROUP_LABELS, PERIOD_LABELS } from './labels';
import { NUTRIENTS } from './nutrients';
import { formatIn, roundTo } from './quantity';

/** Facteur d'affichage et unité des bornes d'un critère (valeurs entières côté serveur, ADR-0001). */
export function scale(c: Criterion): { factor: number; unit: string } {
  switch (c.kind) {
    case 'ENERGY':
      return { factor: 1000, unit: 'kcal' };
    case 'NUTRIENT':
      // « % » suffit : le champ « Exprimé en » dit déjà « part de l'énergie ».
      return c.expression === 'PERCENT_ENERGY'
        ? { factor: 10, unit: '%' }
        : { factor: 1000, unit: 'g' };
    case 'GROUP_PORTIONS':
      return { factor: 1000, unit: 'portions' };
    case 'GROUP_AMOUNT':
      return { factor: 1000, unit: 'g' };
    case 'GROUP_FREQUENCY':
      return { factor: 1, unit: 'repas' };
    case 'RECIPE_REPEAT':
      return { factor: 1, unit: 'jours d’écart' };
    case 'FAMILY_REPEAT':
      return { factor: 1, unit: 'fois' };
  }
}

/** Valeur d'un critère dans son unité d'affichage : « 1 963 kcal », « 36 % », « 4,5 portions ». */
export function formatCriterionValue(c: Criterion, raw: number | null | undefined): string {
  if (raw === null || raw === undefined) {
    return '—';
  }
  const { factor, unit } = scale(c);
  // Précision d'affichage : la kcal, le % et l'occurrence entiers ; le dixième de gramme et de portion.
  const step = unit === 'g' || unit === 'portions' ? factor / 10 : factor;
  return `${formatIn(roundTo(raw, step), { symbol: unit, amount: factor, aliases: [] })}\u00a0${unit}`;
}

/** « Lipides, par jour », « Légumes, Fruits, par jour », « Même recette, par semaine ». */
export function describeCriterion(c: Criterion, familyName?: (id: string) => string): string {
  let target: string = CRITERION_KIND_LABELS[c.kind];
  if (c.kind === 'ENERGY') {
    target = 'Énergie';
  } else if (c.kind === 'NUTRIENT') {
    const label = NUTRIENTS.find((n) => n.code === c.nutrient_code)?.label ?? c.nutrient_code ?? '';
    target = c.expression === 'PERCENT_ENERGY' ? `${label} (part de l’énergie)` : label;
  } else if (c.nutrition_groups?.length) {
    target = `${CRITERION_KIND_LABELS[c.kind]} : ${c.nutrition_groups.map((g) => NUTRITION_GROUP_LABELS[g]).join(', ')}`;
  } else if (c.kind === 'FAMILY_REPEAT' && c.food_group_id && familyName) {
    target = `${CRITERION_KIND_LABELS[c.kind]} : ${familyName(c.food_group_id)}`;
  }
  return `${target}, ${PERIOD_LABELS[c.period]}`;
}
