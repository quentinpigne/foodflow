/**
 * Valeurs nutritionnelles pour 100 g ou 100 mL (ADR-0001) : énergie en cal (millième de kcal), nutriments
 * en mg. On les saisit et les affiche en kcal et en g : le facteur est le même, 1 000.
 */

import { formatIn, parseQuantity, UNITS } from './quantity';

export interface NutrientInfo {
  code: string;
  label: string;
  unit: 'kcal' | 'g';
}

/** Nutriments suivis, dans l'ordre de l'étiquette nutritionnelle. */
export const NUTRIENTS: NutrientInfo[] = [
  { code: 'ENERGY', label: 'Énergie', unit: 'kcal' },
  { code: 'FAT', label: 'Lipides', unit: 'g' },
  { code: 'SATURATED_FAT', label: 'dont acides gras saturés', unit: 'g' },
  { code: 'CARBOHYDRATE', label: 'Glucides', unit: 'g' },
  { code: 'SUGARS', label: 'dont sucres', unit: 'g' },
  { code: 'FIBER', label: 'Fibres', unit: 'g' },
  { code: 'PROTEIN', label: 'Protéines', unit: 'g' },
  { code: 'SALT', label: 'Sel', unit: 'g' },
];

const THOUSANDTHS = UNITS.MASS[0];

/** « 3,2 » g → 3 200 mg ; « 46 » kcal → 46 000 cal. `null` si la saisie est invalide. */
export function parseNutrient(text: string): number | null {
  return parseQuantity(text, 'MASS', THOUSANDTHS);
}

/** 3 200 mg → « 3,2 ». */
export function formatNutrient(value: number): string {
  return formatIn(value, THOUSANDTHS);
}
