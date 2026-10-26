import { Criterion } from '../api/models/criterion';
import { describeCriterion, formatCriterionValue } from './criteria';

const energy: Criterion = {
  kind: 'ENERGY',
  nutrient_code: 'ENERGY',
  expression: 'ABSOLUTE',
  period: 'DAY',
  importance: 'PREFERRED',
};

describe('critères', () => {
  it('s’affichent dans leur unité, arrondis', () => {
    expect(formatCriterionValue(energy, 163_863)).toBe('164 kcal');
    const fat: Criterion = {
      ...energy,
      kind: 'NUTRIENT',
      nutrient_code: 'FAT',
      expression: 'PERCENT_ENERGY',
    };
    expect(formatCriterionValue(fat, 496)).toBe('50 %');
    const fiber: Criterion = { ...energy, kind: 'NUTRIENT', nutrient_code: 'FIBER' };
    expect(formatCriterionValue(fiber, 2_549)).toBe('2,5 g');
    expect(formatCriterionValue(fiber, null)).toBe('—');
  });

  it('se décrivent', () => {
    expect(describeCriterion(energy)).toBe('Énergie, par jour');
    const legumes: Criterion = {
      ...energy,
      kind: 'GROUP_FREQUENCY',
      nutrition_groups: ['LEGUMES'],
      period: 'WEEK',
    };
    expect(describeCriterion(legumes)).toBe('Fréquence d’un groupe : Légumineuses, par semaine');
  });
});
