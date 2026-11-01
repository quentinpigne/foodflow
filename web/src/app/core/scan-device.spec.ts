import { TestBed } from '@angular/core/testing';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ScanDevice } from './scan-device';

/** Stockage du navigateur factice : l'environnement de test n'en fournit pas. */
function fakeStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => void values.delete(key),
    setItem: (key, value) => void values.set(key, value),
  };
}

describe('R-STK-50 moyen de scan de l’appareil', () => {
  beforeEach(() => vi.stubGlobal('localStorage', fakeStorage()));
  afterEach(() => {
    vi.unstubAllGlobals();
    TestBed.resetTestingModule();
  });

  it('garde le choix dans le navigateur', () => {
    TestBed.inject(ScanDevice).set('camera');
    expect(localStorage.getItem('foodflow.scan-device')).toBe('camera');
    TestBed.resetTestingModule();
    expect(TestBed.inject(ScanDevice).mode()).toBe('camera');
    TestBed.inject(ScanDevice).set('reader');
    TestBed.resetTestingModule();
    expect(TestBed.inject(ScanDevice).mode()).toBe('reader');
  });

  it('ignore une valeur inconnue', () => {
    localStorage.setItem('foodflow.scan-device', 'autre');
    expect(['camera', 'reader']).toContain(TestBed.inject(ScanDevice).mode());
  });

  it('marche sans stockage', () => {
    vi.stubGlobal('localStorage', undefined);
    const device = TestBed.inject(ScanDevice);
    device.set('camera');
    expect(device.mode()).toBe('camera');
  });
});
