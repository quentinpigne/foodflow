import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  ElementRef,
  afterNextRender,
  inject,
  output,
  signal,
  viewChild,
} from '@angular/core';
import { RouterLink } from '@angular/router';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';

import {
  AIM,
  CAMERA_CONSTRAINTS,
  Detector,
  READ_ZONE,
  ReaderInput,
  RepeatFilter,
  aimRegion,
  continuousFocus,
  createDetector,
  scanFeedback,
} from '../core/barcode';
import { ScanDevice } from '../core/scan-device';

/**
 * Lecture de codes-barres en rafale (R-STK-50), selon le moyen réglé pour l'appareil : la caméra démarre
 * dès l'affichage ; le lecteur de code-barres est reconnu à sa frappe rapide, où que soit la page. Le code
 * peut toujours être tapé. Chaque lecture est signalée aussitôt (bip, vibration), avant toute recherche.
 */
@Component({
  selector: 'app-barcode-scanner',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: { '(document:keydown)': 'onKey($event)' },
  template: `
    <div class="scanner">
      @if (camera()) {
        <div class="video">
          <video #video muted playsinline></video>
          <div class="aim" [style.inset]="aimInset"></div>
        </div>
      }
      @if (error()) {
        <p class="error">{{ error() }}</p>
      }
      <div class="controls">
        @if (device.mode() === 'camera') {
          <button mat-stroked-button type="button" (click)="toggleCamera()">
            <mat-icon>{{ camera() ? 'videocam_off' : 'photo_camera' }}</mat-icon>
            {{ camera() ? 'Arrêter la caméra' : 'Relancer la caméra' }}
          </button>
        }
        <form class="manual" (ngSubmit)="submitManual()">
          <mat-form-field subscriptSizing="dynamic">
            <mat-label>Code-barres</mat-label>
            <input
              matInput
              name="code"
              inputmode="numeric"
              autocomplete="off"
              [ngModel]="manual()"
              (ngModelChange)="manual.set($event)"
            />
            <button mat-icon-button matIconSuffix type="submit" aria-label="Ajouter le code">
              <mat-icon>keyboard_return</mat-icon>
            </button>
          </mat-form-field>
        </form>
      </div>
      <p class="muted hint">
        {{
          device.mode() === 'camera'
            ? 'Scan par la caméra de cet appareil.'
            : 'Lecteur de code-barres prêt : scanner directement, sans clic.'
        }}
        <a routerLink="/reglages">Changer</a>
      </p>
    </div>
  `,
  styles: `
    .scanner {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
    .video {
      position: relative;
      overflow: hidden;
      border-radius: var(--mat-sys-corner-medium);
      background: #000;
      max-height: 40vh;
    }
    video {
      display: block;
      width: 100%;
      max-height: 40vh;
      object-fit: cover;
    }
    .aim {
      position: absolute;
      border: 2px solid rgb(255 255 255 / 70%);
      border-radius: 8px;
      pointer-events: none;
    }
    .controls {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
    }
    .manual {
      flex: 1;
      min-width: 12rem;
    }
    .manual mat-form-field {
      width: 100%;
    }
    .hint {
      margin: 0;
    }
    .error {
      color: var(--mat-sys-error);
      margin: 0;
    }
  `,
})
export class BarcodeScanner {
  readonly scanned = output<string>();
  protected readonly device = inject(ScanDevice);

  private readonly video = viewChild<ElementRef<HTMLVideoElement>>('video');
  protected readonly camera = signal(false);
  protected readonly manual = signal('');
  protected readonly error = signal<string | null>(null);
  protected readonly aimInset = `${AIM.y * 100}% ${AIM.x * 100}%`;

  private readonly reader = new ReaderInput();
  private readonly repeats = new RepeatFilter();
  private stream: MediaStream | null = null;
  private detector: Detector | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  /** Image de la zone décodée, copiée de la vidéo à chaque passage. */
  private canvas: HTMLCanvasElement | null = null;

  constructor() {
    inject(DestroyRef).onDestroy(() => this.stop());
    // En mode caméra, elle démarre dès l'ouverture du scan, sans second clic.
    afterNextRender(() => {
      if (this.device.mode() === 'camera') {
        void this.toggleCamera();
      }
    });
  }

  /** Lecteur de code-barres : il tape comme un clavier ; ignoré quand on saisit dans un champ. */
  protected onKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement | null;
    if (
      target &&
      (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
    ) {
      return;
    }
    const code = this.reader.key(event.key, event.timeStamp);
    if (code) {
      event.preventDefault();
      this.emit(code);
    }
  }

  /** Signale la lecture tout de suite, puis la transmet (R-STK-50). */
  private emit(code: string): void {
    scanFeedback('read');
    this.scanned.emit(code);
  }

  protected submitManual(): void {
    const code = this.manual().replace(/\s/g, '');
    if (code) {
      this.emit(code);
      this.manual.set('');
    }
  }

  protected async toggleCamera(): Promise<void> {
    if (this.camera()) {
      this.stop();
      return;
    }
    this.error.set(null);
    // Le décodeur se prépare pendant que la caméra s'ouvre.
    const detector = this.detector ? Promise.resolve(this.detector) : createDetector();
    try {
      this.stream = await navigator.mediaDevices.getUserMedia(CAMERA_CONSTRAINTS);
      const track = this.stream.getVideoTracks()[0];
      if (track) {
        await continuousFocus(track);
      }
      this.camera.set(true);
      this.detector = await detector;
      // La vidéo n'existe qu'après l'affichage du bloc caméra.
      await new Promise((resolve) => setTimeout(resolve));
      const video = this.video()?.nativeElement;
      if (!video || !this.stream) {
        return;
      }
      video.srcObject = this.stream;
      await video.play();
      this.loop();
    } catch {
      this.stop();
      this.error.set(
        'Caméra indisponible : autoriser son accès (l’application doit être servie en HTTPS), ou utiliser un lecteur de code-barres.',
      );
    }
  }

  private loop(): void {
    this.timer = setTimeout(async () => {
      const video = this.video()?.nativeElement;
      if (!this.camera() || !video || !this.detector) {
        return;
      }
      try {
        const zone = this.zone(video);
        for (const code of zone ? await this.detector.detect(zone) : []) {
          if (this.repeats.accept(code.rawValue, performance.now())) {
            this.emit(code.rawValue);
          }
        }
      } catch {
        // Image illisible : on passe à la suivante.
      }
      this.loop();
    }, 150);
  }

  /** Copie la zone du cadre de visée (élargie) : seule elle est décodée. */
  private zone(video: HTMLVideoElement): HTMLCanvasElement | null {
    if (!video.videoWidth || !video.videoHeight) {
      return null;
    }
    const region = aimRegion(
      { width: video.videoWidth, height: video.videoHeight },
      { width: video.clientWidth, height: video.clientHeight },
      READ_ZONE,
    );
    this.canvas ??= document.createElement('canvas');
    const canvas = this.canvas;
    if (canvas.width !== region.width || canvas.height !== region.height) {
      canvas.width = region.width;
      canvas.height = region.height;
    }
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) {
      return null;
    }
    context.drawImage(
      video,
      region.x,
      region.y,
      region.width,
      region.height,
      0,
      0,
      region.width,
      region.height,
    );
    return canvas;
  }

  private stop(): void {
    if (this.timer) {
      clearTimeout(this.timer);
    }
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.camera.set(false);
  }
}
