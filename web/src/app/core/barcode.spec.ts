import { describe, expect, it } from 'vitest';

import { ReaderInput, RepeatFilter, aimRegion } from './barcode';

function type(reader: ReaderInput, keys: string[], start: number, gap: number): (string | null)[] {
  return keys.map((k, i) => reader.key(k, start + i * gap));
}

describe('R-STK-50 lecteur de code-barres', () => {
  it('reconnaît une frappe rapide terminée par Entrée', () => {
    const reader = new ReaderInput();
    const results = type(reader, [...'3017620422003', 'Enter'], 0, 10);
    expect(results.at(-1)).toBe('3017620422003');
    expect(results.slice(0, -1).every((r) => r === null)).toBe(true);
  });

  it('ignore une frappe humaine', () => {
    const reader = new ReaderInput();
    expect(type(reader, [...'3017620422003', 'Enter'], 0, 200).at(-1)).toBeNull();
  });

  it('ignore un code trop court ou mêlé de lettres', () => {
    const reader = new ReaderInput();
    expect(type(reader, [...'1234', 'Enter'], 0, 10).at(-1)).toBeNull();
    expect(type(reader, [...'30176a0422003', 'Enter'], 1000, 10).at(-1)).toBeNull();
  });

  it('lit deux codes à la suite', () => {
    const reader = new ReaderInput();
    expect(type(reader, [...'96385074', 'Enter'], 0, 10).at(-1)).toBe('96385074');
    expect(type(reader, [...'96385074', 'Enter'], 500, 10).at(-1)).toBe('96385074');
  });
});

describe('R-STK-50 caméra', () => {
  it('compte une seule fois un code qui reste dans le champ', () => {
    const filter = new RepeatFilter();
    expect(filter.accept('96385074', 0)).toBe(true);
    // Relu toutes les 150 ms pendant 3 secondes.
    for (let t = 150; t <= 3000; t += 150) {
      expect(filter.accept('96385074', t)).toBe(false);
    }
    expect(filter.accept('3017620422003', 3100)).toBe(true);
  });

  it('relit un code absent du champ depuis plus de 2 secondes', () => {
    const filter = new RepeatFilter();
    filter.accept('96385074', 0);
    expect(filter.accept('96385074', 2500)).toBe(true);
  });
});

describe('R-STK-50 zone décodée par la caméra', () => {
  it('reporte le cadre affiché sur l’image de la caméra, rognée pour remplir la vidéo', () => {
    // Image 1280×720 affichée dans 400×300 : rognée de 160 px à gauche et à droite.
    expect(
      aimRegion({ width: 1280, height: 720 }, { width: 400, height: 300 }, { x: 0.05, y: 0.25 }),
    ).toEqual({ x: 208, y: 180, width: 864, height: 360 });
  });

  it('décode toute l’image si la vidéo n’est pas affichée', () => {
    expect(
      aimRegion({ width: 640, height: 480 }, { width: 0, height: 0 }, { x: 0.05, y: 0.25 }),
    ).toEqual({ x: 0, y: 0, width: 640, height: 480 });
  });
});
