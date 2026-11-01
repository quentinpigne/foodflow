/** Mise en stock et inventaire par scan (STK-12, STK-13) : ce que devient chaque lecture. */

import { BarcodeLookup } from '../../api/models/barcode-lookup';
import { ExpiryKind } from '../../api/models/expiry-kind';
import { Lot } from '../../api/models/lot';
import { OffStatus } from '../../api/models/off-status';
import { Product } from '../../api/models/product';
import { ProductDraft } from '../../api/models/product-draft';
import { TrackingMode } from '../../api/models/tracking-mode';
import { quantityInput } from '../../core/quantity';

/** Ligne de la mise en stock par scan : un produit (ou un code à rattacher) et ses lots proposés. */
export interface EntryLine {
  barcode: string;
  /** `null` : code inconnu, à rattacher (R-STK-52). */
  product: Product | null;
  tracking: TrackingMode;
  /** Nombre de conditionnements scannés : autant de lots (R-STK-51, R-STK-53). */
  count: number;
  location: string | null;
  /** Quantité d'un lot, saisie (« 500 g ») ; le conditionnement par défaut. */
  quantity: string;
  expiryKind: ExpiryKind;
  /** `AAAA-MM-JJ`, ou `null` : à compléter (R-COU-51). */
  expiryDate: string | null;
  /** Code inconnu : brouillon d'Open Food Facts à valider (R-ALI-60), et ce qu'a répondu le service. */
  draft?: ProductDraft | null;
  offStatus?: OffStatus | null;
  /** Code lu dont la recherche (base, puis Open Food Facts) n'a pas encore répondu. */
  pending?: boolean;
}

/** Ligne proposée pour un code lu (R-STK-53). */
export function newLine(lookup: BarcodeLookup, defaultLocation: string | null): EntryLine {
  const product = lookup.product ?? null;
  return {
    barcode: lookup.barcode,
    product,
    tracking: lookup.tracking_mode ?? 'QUANTITY',
    count: 1,
    location: lookup.location_id ?? defaultLocation,
    quantity: product?.package_amount
      ? quantityInput(product.package_amount, product.stock_measure)
      : '',
    expiryKind: lookup.expiry_kind ?? 'USE_BY',
    expiryDate: null,
    draft: lookup.draft ?? null,
    offStatus: lookup.off_status ?? null,
  };
}

/**
 * Lecture d'un code déjà présent : un conditionnement de plus, sans redemander le code au serveur (ni à
 * Open Food Facts). `null` si le code n'a pas encore de ligne.
 */
export function addAgain(lines: EntryLine[], barcode: string): EntryLine[] | null {
  const existing = lines.find((l) => l.barcode === barcode);
  return existing
    ? [{ ...existing, count: existing.count + 1 }, ...lines.filter((l) => l !== existing)]
    : null;
}

/**
 * Ligne affichée dès la lecture d'un nouveau code, avant la réponse de la recherche (R-STK-50) : les
 * lectures suivantes du même code s'y ajoutent.
 */
export function pendingLine(barcode: string, defaultLocation: string | null): EntryLine {
  return {
    barcode,
    product: null,
    tracking: 'QUANTITY',
    count: 1,
    location: defaultLocation,
    quantity: '',
    expiryKind: 'USE_BY',
    expiryDate: null,
    pending: true,
  };
}

/**
 * Complète la ligne en attente du code lu avec la réponse de la recherche, en gardant le nombre de
 * lectures. Rien ne change si la ligne a été retirée entre-temps.
 */
export function resolveLine(
  lines: EntryLine[],
  code: string,
  lookup: BarcodeLookup,
  defaultLocation: string | null,
): EntryLine[] {
  return lines.map((l) =>
    l.barcode === code && l.pending
      ? { ...newLine(lookup, l.location ?? defaultLocation), count: l.count }
      : l,
  );
}

/**
 * Ajoute une lecture (R-STK-51) : un conditionnement de plus sur la ligne du même code, remontée en tête,
 * ou une nouvelle ligne en tête.
 */
export function addScan(
  lines: EntryLine[],
  lookup: BarcodeLookup,
  defaultLocation: string | null,
): EntryLine[] {
  return addAgain(lines, lookup.barcode) ?? [newLine(lookup, defaultLocation), ...lines];
}

/** Inventaire par scan en cours (R-STK-55). */
export interface InventoryScan {
  /** Lots constatés, dans l'ordre des lectures. */
  observed: string[];
  /** Lots trouvés en plus, par produit : nombre de conditionnements. */
  found: Record<string, number>;
}

export const EMPTY_INVENTORY: InventoryScan = { observed: [], found: {} };

/**
 * Lecture d'un produit pendant l'inventaire d'un emplacement (R-STK-55) : elle constate le lot non
 * épuisé de ce produit, pas encore constaté, le plus proche de sa date limite (les lots sont triés par
 * date limite effective) ; à défaut, elle compte un lot trouvé.
 */
export function assignScan(
  scan: InventoryScan,
  productId: string,
  lots: readonly Lot[],
): InventoryScan {
  const lot = lots.find(
    (l) => l.product_id === productId && !l.depleted && !scan.observed.includes(l.id),
  );
  if (lot) {
    return { ...scan, observed: [...scan.observed, lot.id] };
  }
  return { ...scan, found: { ...scan.found, [productId]: (scan.found[productId] ?? 0) + 1 } };
}
