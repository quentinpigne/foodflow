/** Mise en stock et inventaire par scan (STK-12, STK-13) : ce que devient chaque lecture. */

import { BarcodeLookup } from '../../api/models/barcode-lookup';
import { ExpiryKind } from '../../api/models/expiry-kind';
import { Lot } from '../../api/models/lot';
import { Product } from '../../api/models/product';
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
  };
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
  const existing = lines.find((l) => l.barcode === lookup.barcode);
  if (existing) {
    return [{ ...existing, count: existing.count + 1 }, ...lines.filter((l) => l !== existing)];
  }
  return [newLine(lookup, defaultLocation), ...lines];
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
