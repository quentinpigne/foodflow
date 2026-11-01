/** Lecture des codes-barres (R-STK-50) : caméra, lecteur de code-barres, retour sonore. */

/** Délai pendant lequel la caméra ignore un code déjà lu (R-STK-50). */
export const CAMERA_REPEAT_MS = 2000;

/** Écart maximal entre deux touches tapées par un lecteur de code-barres. */
const READER_GAP_MS = 60;

/**
 * Reconnaît un lecteur de code-barres, qui tape les chiffres très vite puis Entrée. Une frappe humaine,
 * plus lente, remet la lecture à zéro.
 */
export class ReaderInput {
  private buffer = '';
  private last = Number.NEGATIVE_INFINITY;

  /** Renvoie le code lu quand une frappe rapide d'au moins 8 chiffres se termine par Entrée. */
  key(key: string, time: number): string | null {
    if (time - this.last > READER_GAP_MS) {
      this.buffer = '';
    }
    this.last = time;
    if (key === 'Enter') {
      const code = this.buffer;
      this.buffer = '';
      return code.length >= 8 ? code : null;
    }
    this.buffer = /^\d$/.test(key) ? this.buffer + key : '';
    return null;
  }
}

/**
 * Filtre les lectures répétées de la caméra : un code qui reste dans le champ est relu à chaque image, il
 * ne compte qu'une fois tant qu'il n'a pas quitté le champ pendant `CAMERA_REPEAT_MS`.
 */
export class RepeatFilter {
  private readonly seen = new Map<string, number>();

  accept(code: string, time: number): boolean {
    const last = this.seen.get(code);
    this.seen.set(code, time);
    return last === undefined || time - last > CAMERA_REPEAT_MS;
  }
}

/** Ce que l'on fait d'un code lu : connu, inconnu (à rattacher), refusé (clé fausse). */
export type ScanOutcome = 'known' | 'unknown' | 'invalid';

let audio: AudioContext | null = null;

/** Bip et vibration distincts selon le résultat d'une lecture (R-STK-50). */
export function scanFeedback(outcome: ScanOutcome): void {
  const tones: Record<ScanOutcome, [number, number][]> = {
    known: [[1320, 0.08]],
    unknown: [
      [880, 0.08],
      [1320, 0.08],
    ],
    invalid: [[220, 0.25]],
  };
  const vibration: Record<ScanOutcome, number[]> = {
    known: [40],
    unknown: [40, 60, 40],
    invalid: [200],
  };
  try {
    navigator.vibrate?.(vibration[outcome]);
    audio ??= new AudioContext();
    let start = audio.currentTime;
    for (const [frequency, duration] of tones[outcome]) {
      const oscillator = audio.createOscillator();
      const gain = audio.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.value = 0.15;
      oscillator.connect(gain).connect(audio.destination);
      oscillator.start(start);
      oscillator.stop(start + duration);
      start += duration + 0.04;
    }
  } catch {
    // Pas de son ni de vibration : la ligne affichée suffit.
  }
}

/** Détecteur de codes-barres : l'API du navigateur, sinon la bibliothèque de repli (ADR-0009). */
export interface Detector {
  detect(source: HTMLVideoElement): Promise<{ rawValue: string }[]>;
}

const FORMATS = ['ean_13', 'ean_8', 'upc_a'];

interface NativeDetectorClass {
  new (options: { formats: string[] }): Detector;
  getSupportedFormats(): Promise<string[]>;
}

export async function createDetector(): Promise<Detector> {
  const native = (globalThis as { BarcodeDetector?: NativeDetectorClass }).BarcodeDetector;
  if (native) {
    const supported = await native.getSupportedFormats();
    if (FORMATS.every((f) => supported.includes(f))) {
      return new native({ formats: FORMATS });
    }
  }
  const { BarcodeDetector, prepareZXingModule } = await import('barcode-detector/ponyfill');
  // Le module WebAssembly est servi par Foodflow : aucun appel extérieur (ADR-0007).
  prepareZXingModule({
    overrides: {
      locateFile: (path: string, prefix: string) =>
        path.endsWith('.wasm') ? new URL(`zxing/${path}`, document.baseURI).href : prefix + path,
    },
  });
  return new BarcodeDetector({ formats: FORMATS as ('ean_13' | 'ean_8' | 'upc_a')[] });
}
