import { describe, expect, it } from 'vitest';

import { BarcodeLookup } from '../../api/models/barcode-lookup';
import { Lot } from '../../api/models/lot';
import { Product } from '../../api/models/product';
import { EMPTY_INVENTORY, EntryLine, addScan, assignScan } from './scan';

const panzani: Product = {
  id: 'panzani',
  food_id: 'pates',
  attached_to: 'Pâtes',
  name: 'Panzani spaghetti',
  barcode: '3017620422003',
  stock_measure: 'MASS',
  package_amount: 500_000,
  nutrient_basis: 'PER_100_G',
};

const known: BarcodeLookup = {
  barcode: '3017620422003',
  product: panzani,
  tracking_mode: 'QUANTITY',
  location_id: 'placard',
  expiry_kind: 'BEST_BEFORE',
};

function lot(id: string, product: string, expiry: string): Lot {
  return {
    id,
    product_id: product,
    name: product,
    location_id: 'placard',
    location_name: 'Placard',
    location_kind: 'PANTRY',
    tracking_mode: 'QUANTITY',
    measure: 'MASS',
    quantity: 500_000,
    expiry_kind: 'USE_BY',
    expiry_date: expiry,
    expiry_missing: false,
    status: 'OK',
    stocked_on: '2026-10-25',
    needs_review: false,
    depleted: false,
  } as Lot;
}

describe('R-STK-51 R-STK-53 mise en stock par scan', () => {
  it('trois lectures du même produit donnent une ligne de 3 conditionnements de 500 g', () => {
    let lines: EntryLine[] = [];
    for (let i = 0; i < 3; i++) {
      lines = addScan(lines, known, 'frigo');
    }
    expect(lines).toHaveLength(1);
    expect(lines[0].count).toBe(3);
    expect(lines[0].quantity).toBe('500 g');
    expect(lines[0].location).toBe('placard');
    expect(lines[0].expiryKind).toBe('BEST_BEFORE');
    expect(lines[0].expiryDate).toBeNull();
  });

  it('un code inconnu donne une ligne à rattacher, à l’emplacement de la séance', () => {
    const lines = addScan([], { barcode: '96385074' }, 'frigo');
    expect(lines[0].product).toBeNull();
    expect(lines[0].location).toBe('frigo');
    expect(lines[0].expiryKind).toBe('USE_BY');
  });

  it('la dernière ligne lue remonte en tête', () => {
    let lines = addScan([], known, null);
    lines = addScan(lines, { barcode: '96385074' }, null);
    lines = addScan(lines, known, null);
    expect(lines.map((l) => l.barcode)).toEqual(['3017620422003', '96385074']);
  });
});

describe('R-STK-55 inventaire par scan', () => {
  // Critère d'acceptation de la spec 02 : deux lots au placard, produit scanné trois fois.
  const lots = [lot('l1', 'panzani', '2026-11-10'), lot('l2', 'panzani', '2026-12-20')];

  it('constate les lots du plus proche au plus lointain, puis compte un lot trouvé', () => {
    let scan = EMPTY_INVENTORY;
    for (let i = 0; i < 3; i++) {
      scan = assignScan(scan, 'panzani', lots);
    }
    expect(scan.observed).toEqual(['l1', 'l2']);
    expect(scan.found).toEqual({ panzani: 1 });
  });

  it('un produit sans lot à l’emplacement donne un lot trouvé', () => {
    expect(assignScan(EMPTY_INVENTORY, 'barilla', lots).found).toEqual({ barilla: 1 });
  });
});
