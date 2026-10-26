/**
 * Quantités en unités de stockage (ADR-0001) : mg, µL ou millième de pièce. Analyse des saisies
 * (« 1,5 kg », « 250 mL », « 6 ») et formatage.
 *
 * Seul module autorisé à faire de l'arithmétique sur des quantités (CLAUDE.md). Aucun calcul ne passe par
 * un flottant : les quantités restent des entiers, manipulés comme tels.
 */

import { Measure } from '../api/models/measure';

export interface QuantityUnit {
  /** Symbole affiché et saisi (« kg »). */
  symbol: string;
  /** Valeur d'une unité en unités de stockage (1 kg = 1 000 000 mg). */
  amount: number;
  /** Autres écritures acceptées à la saisie. */
  aliases: string[];
}

/** Unités de saisie et d'affichage de chaque mesure, de la plus petite à la plus grande. */
export const UNITS: Record<Measure, QuantityUnit[]> = {
  MASS: [
    { symbol: 'g', amount: 1_000, aliases: ['gr', 'gramme', 'grammes'] },
    { symbol: 'kg', amount: 1_000_000, aliases: ['kilo', 'kilos', 'kilogramme', 'kilogrammes'] },
  ],
  VOLUME: [
    { symbol: 'mL', amount: 1_000, aliases: ['ml', 'millilitre', 'millilitres'] },
    { symbol: 'cL', amount: 10_000, aliases: ['cl', 'centilitre', 'centilitres'] },
    { symbol: 'L', amount: 1_000_000, aliases: ['l', 'litre', 'litres'] },
  ],
  COUNT: [
    { symbol: 'pièce', amount: 1_000, aliases: ['pièces', 'piece', 'pieces', 'pc', 'pcs', 'p'] },
  ],
};

/** Unité de saisie par défaut d'une mesure : g, mL, pièce. */
export function defaultUnit(measure: Measure): QuantityUnit {
  return UNITS[measure][0];
}

function findUnit(measure: Measure, text: string): QuantityUnit | null {
  const key = text.trim().toLowerCase();
  return (
    UNITS[measure].find((u) => u.symbol.toLowerCase() === key || u.aliases.includes(key)) ?? null
  );
}

const SPACES = /[\s\u00a0\u202f]/g;

/**
 * Analyse une saisie (« 1,5 kg », « 250 », « 2 L ») dans une mesure. Sans unité, l'unité donnée (ou celle
 * par défaut de la mesure) s'applique. Renvoie `null` si la saisie n'est pas une quantité positive ou
 * nulle valide, ou si elle est plus précise que l'unité de stockage.
 */
export function parseQuantity(
  input: string,
  measure: Measure,
  unit: QuantityUnit = defaultUnit(measure),
): number | null {
  const match = /^\s*(\d[\d\s\u00a0\u202f]*)(?:[.,](\d+))?\s*([^\d\s].*)?$/.exec(input);
  if (!match) {
    return null;
  }
  const [, integerPart, decimals = '', unitText] = match;
  const chosen = unitText ? findUnit(measure, unitText) : unit;
  if (!chosen) {
    return null;
  }
  const integer = Number(integerPart.replace(SPACES, ''));
  const scale = 10 ** decimals.length;
  // La partie décimale doit tomber juste en unités de stockage : 1,5 g = 1 500 mg, mais 1,5 mg n'existe pas.
  if ((Number(decimals || '0') * chosen.amount) % scale !== 0) {
    return null;
  }
  return integer * chosen.amount + (Number(decimals || '0') * chosen.amount) / scale;
}

/** Valeur d'une quantité dans une unité, avec au plus trois décimales (« 1,25 »). */
export function formatIn(raw: number, unit: QuantityUnit): string {
  const negative = raw < 0;
  const absolute = Math.abs(raw);
  const integer = Math.trunc(absolute / unit.amount);
  const rest = absolute % unit.amount;
  // Trois décimales au plus, arrondies au plus proche, sans zéros inutiles.
  let thousandths = Math.round((rest * 1_000) / unit.amount);
  let whole = integer;
  if (thousandths === 1_000) {
    whole += 1;
    thousandths = 0;
  }
  const grouped = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 0 }).format(whole);
  const fraction =
    thousandths === 0 ? '' : ',' + String(thousandths).padStart(3, '0').replace(/0+$/, '');
  return `${negative ? '−' : ''}${grouped}${fraction}`;
}

/** Unité la mieux adaptée à l'affichage : la plus grande qui donne au moins 1. */
export function bestUnit(raw: number, measure: Measure): QuantityUnit {
  const units = UNITS[measure];
  const absolute = Math.abs(raw);
  return [...units].reverse().find((u) => absolute >= u.amount) ?? units[0];
}

/** « 1,5 kg », « 250 mL », « 3 pièces ». */
export function formatQuantity(raw: number | null | undefined, measure: Measure): string {
  if (raw === null || raw === undefined) {
    return '—';
  }
  const unit = bestUnit(raw, measure);
  const text = formatIn(raw, unit);
  const symbol = measure === 'COUNT' && Math.abs(raw) > unit.amount ? 'pièces' : unit.symbol;
  return `${text}\u00a0${symbol}`;
}

/** Valeur à proposer dans un champ de saisie : « 1,5 kg ». */
export function quantityInput(raw: number, measure: Measure): string {
  const unit = bestUnit(raw, measure);
  return `${formatIn(raw, unit)} ${unit.symbol}`;
}

/** « ≈ 1 ½ » : une quantité de pièces déjà arrondie à la demi-pièce (R-REC-14). */
export function formatHalves(raw: number): string {
  const unit = UNITS.COUNT[0];
  if (raw % unit.amount !== unit.amount / 2) {
    return formatIn(raw, unit);
  }
  return raw < unit.amount ? '½' : `${formatIn(raw - unit.amount / 2, unit)} ½`;
}

/** Portions, en millièmes : « 2,6 ». */
export function formatPortions(raw: number | null | undefined): string {
  return raw === null || raw === undefined ? '—' : formatIn(raw, UNITS.COUNT[0]);
}

/** « 2,6 » → 2 600 millièmes de portion ; `null` si la saisie n'est pas valide. */
export function parsePortions(text: string): number | null {
  return parseQuantity(text, 'COUNT');
}

/** Somme de quantités, de portions ou de coefficients (entiers en millièmes). */
export function total(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0);
}

/** La plus petite de deux quantités. */
export function smallest(a: number, b: number): number {
  return Math.min(a, b);
}

/** Différence de deux quantités (ce qui reste à couvrir), jamais négative. */
export function remainder(wanted: number, covered: number): number {
  return Math.max(0, wanted - covered);
}

/** Part en millièmes → pourcentage entier (« 667 » → 67). */
export function percent(ratio: number): number {
  return Math.round(ratio / 10);
}

/** Arrondit une quantité au multiple de `step` le plus proche (affichage : 163 863 cal → 164 000). */
export function roundTo(raw: number, step: number): number {
  return Math.round(raw / step) * step;
}
