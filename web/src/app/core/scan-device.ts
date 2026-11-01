import { Injectable, signal } from '@angular/core';

/** Moyen de lecture des codes-barres de cet appareil (R-STK-50). */
export type ScanDeviceMode = 'camera' | 'reader';

const KEY = 'foodflow.scan-device';

/**
 * Réglage propre à l'appareil (téléphone : caméra ; ordinateur : lecteur de code-barres), gardé dans le
 * navigateur. Par défaut : la caméra sur un écran tactile étroit, le lecteur ailleurs.
 */
@Injectable({ providedIn: 'root' })
export class ScanDevice {
  readonly mode = signal<ScanDeviceMode>(load());

  set(mode: ScanDeviceMode): void {
    this.mode.set(mode);
    try {
      localStorage.setItem(KEY, mode);
    } catch {
      // Stockage indisponible : le réglage vaut jusqu'à la fermeture de la page.
    }
  }
}

function load(): ScanDeviceMode {
  try {
    const stored = localStorage.getItem(KEY);
    if (stored === 'camera' || stored === 'reader') {
      return stored;
    }
  } catch {
    // Stockage indisponible : réglage par défaut.
  }
  return defaultMode();
}

export function defaultMode(): ScanDeviceMode {
  const touch = window.matchMedia?.('(pointer: coarse)').matches ?? false;
  return touch ? 'camera' : 'reader';
}
