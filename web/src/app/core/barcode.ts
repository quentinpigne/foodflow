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

/**
 * Signal d'une lecture : `read` dès que le code est lu, avant toute recherche ; puis `unknown` ou `invalid`
 * quand la recherche le demande (un produit connu n'a pas de second signal).
 */
export type ScanSignal = 'read' | 'unknown' | 'invalid';

let audio: AudioContext | null = null;

/** Bip et vibration distincts selon le signal (R-STK-50). */
export function scanFeedback(signal: ScanSignal): void {
  const tones: Record<ScanSignal, [number, number][]> = {
    read: [[1320, 0.08]],
    unknown: [
      [880, 0.08],
      [1320, 0.08],
    ],
    invalid: [[220, 0.25]],
  };
  const vibration: Record<ScanSignal, number[]> = {
    read: [50],
    unknown: [40, 60, 40],
    invalid: [200],
  };
  try {
    navigator.vibrate?.(vibration[signal]);
  } catch {
    // Pas de vibration (iPhone, ordinateur) : le bip et la ligne affichée suffisent.
  }
  try {
    audio ??= new AudioContext();
    if (audio.state === 'suspended') {
      void audio.resume();
    }
    let start = audio.currentTime;
    for (const [frequency, duration] of tones[signal]) {
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
    // Pas de son : la vibration et la ligne affichée suffisent.
  }
}

/** Image demandée à la caméra : assez fine pour lire un code-barres de près (R-STK-50). */
export const CAMERA_CONSTRAINTS: MediaStreamConstraints = {
  video: {
    facingMode: 'environment',
    width: { ideal: 1280 },
    height: { ideal: 720 },
  },
  audio: false,
};

/** Demande la mise au point continue, si la caméra la propose (sinon, rien). */
export async function continuousFocus(track: MediaStreamTrack): Promise<void> {
  const capabilities = (track.getCapabilities?.() ?? {}) as { focusMode?: string[] };
  if (capabilities.focusMode?.includes('continuous')) {
    try {
      await track.applyConstraints({
        advanced: [{ focusMode: 'continuous' } as MediaTrackConstraintSet],
      });
    } catch {
      // Réglage refusé : la caméra garde sa mise au point.
    }
  }
}

/** Marges du cadre de visée affiché, en fractions de la vidéo affichée (haut et bas, gauche et droite). */
export const AIM = { y: 0.35, x: 0.1 };

/** Zone décodée : le cadre de visée, élargi pour pardonner un code mal centré. */
export const READ_ZONE = { y: 0.25, x: 0.05 };

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Rectangle de l'image de la caméra qui correspond à une zone de la vidéo affichée (marges `inset`), la
 * vidéo remplissant son cadre en étant rognée (`object-fit: cover`). Seul ce rectangle est décodé : moins
 * de pixels, donc une lecture plus rapide (R-STK-50). Image entière si la vidéo n'est pas affichée.
 */
export function aimRegion(
  frame: { width: number; height: number },
  shown: { width: number; height: number },
  inset: { x: number; y: number },
): Rect {
  if (!shown.width || !shown.height) {
    return { x: 0, y: 0, width: frame.width, height: frame.height };
  }
  const scale = Math.max(shown.width / frame.width, shown.height / frame.height);
  const offsetX = (frame.width * scale - shown.width) / 2;
  const offsetY = (frame.height * scale - shown.height) / 2;
  const clamp = (v: number, max: number) => Math.min(Math.max(Math.round(v), 0), max);
  const left = clamp((shown.width * inset.x + offsetX) / scale, frame.width);
  const right = clamp((shown.width * (1 - inset.x) + offsetX) / scale, frame.width);
  const top = clamp((shown.height * inset.y + offsetY) / scale, frame.height);
  const bottom = clamp((shown.height * (1 - inset.y) + offsetY) / scale, frame.height);
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** Détecteur de codes-barres : l'API du navigateur, sinon la bibliothèque de repli (ADR-0009). */
export interface Detector {
  detect(source: HTMLCanvasElement): Promise<{ rawValue: string }[]>;
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
