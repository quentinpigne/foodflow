import { formatHalves, formatQuantity, parseQuantity, quantityInput, UNITS } from './quantity';

describe('quantity', () => {
  it('analyse les saisies en unités de stockage (ADR-0001)', () => {
    expect(parseQuantity('1,5 kg', 'MASS')).toBe(1_500_000);
    expect(parseQuantity('250', 'MASS')).toBe(250_000);
    expect(parseQuantity('2 L', 'VOLUME')).toBe(2_000_000);
    expect(parseQuantity('20 cl', 'VOLUME')).toBe(200_000);
    expect(parseQuantity('6', 'COUNT')).toBe(6_000);
    expect(parseQuantity('1 250 g', 'MASS')).toBe(1_250_000);
    expect(parseQuantity('0,5', 'VOLUME', UNITS.VOLUME[2])).toBe(500_000);
  });

  it('refuse les saisies invalides ou trop précises', () => {
    expect(parseQuantity('', 'MASS')).toBeNull();
    expect(parseQuantity('abc', 'MASS')).toBeNull();
    expect(parseQuantity('2 L', 'MASS')).toBeNull();
    expect(parseQuantity('1,2345 g', 'MASS')).toBeNull();
    expect(parseQuantity('-3', 'MASS')).toBeNull();
  });

  it("affiche dans l'unité la plus lisible", () => {
    expect(formatQuantity(1_500_000, 'MASS')).toBe('1,5\u00a0kg');
    expect(formatQuantity(750_000, 'MASS')).toBe('750\u00a0g');
    expect(formatQuantity(550_000, 'VOLUME')).toBe('55\u00a0cL');
    expect(formatQuantity(1_000_000, 'VOLUME')).toBe('1\u00a0L');
    expect(formatQuantity(3_000, 'COUNT')).toBe('3\u00a0pièces');
    expect(formatQuantity(1_000, 'COUNT')).toBe('1\u00a0pièce');
    expect(formatQuantity(2_500, 'COUNT')).toBe('2,5\u00a0pièces');
    expect(formatQuantity(0, 'MASS')).toBe('0\u00a0g');
    expect(formatQuantity(null, 'MASS')).toBe('—');
  });

  it('propose une saisie relisible', () => {
    expect(quantityInput(1_250_000, 'MASS')).toBe('1,25 kg');
    expect(parseQuantity(quantityInput(1_250_000, 'MASS'), 'MASS')).toBe(1_250_000);
  });

  it('affiche les demi-pièces (R-REC-14)', () => {
    expect(formatHalves(1_500)).toBe('1 ½');
    expect(formatHalves(500)).toBe('½');
    expect(formatHalves(2_000)).toBe('2');
  });
});
