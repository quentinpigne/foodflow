import {
  addDays,
  addMonths,
  formatDate,
  formatDayHeader,
  fromIsoDate,
  toIsoDate,
  weekdayOf,
  weekStart,
} from './dates';

describe('dates', () => {
  it('convertit sans glissement de fuseau', () => {
    expect(toIsoDate(fromIsoDate('2026-03-12'))).toBe('2026-03-12');
    expect(toIsoDate(new Date(2026, 0, 31))).toBe('2026-01-31');
  });

  it('formate à la française', () => {
    expect(formatDate('2026-03-12')).toBe('12 mars 2026');
    expect(formatDayHeader('2026-03-12')).toBe('Jeudi 12 mars');
  });
});

describe('semaine', () => {
  it('commence le lundi', () => {
    expect(weekStart('2026-10-25')).toBe('2026-10-19');
    expect(weekStart('2026-10-26')).toBe('2026-10-26');
    expect(addDays('2026-10-31', 1)).toBe('2026-11-01');
    expect(weekdayOf('2026-10-25')).toBe(7);
    expect(weekdayOf('2026-10-26')).toBe(1);
  });

  it('décale d’un mois en restant dans le mois visé', () => {
    expect(addMonths('2026-11-01', 1)).toBe('2026-12-01');
    expect(addMonths('2026-01-31', 1)).toBe('2026-02-28');
    expect(addMonths('2026-12-15', 1)).toBe('2027-01-15');
  });
});
