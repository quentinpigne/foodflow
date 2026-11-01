import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { MatRadioModule } from '@angular/material/radio';

import { ScanDevice, ScanDeviceMode } from '../../core/scan-device';

/**
 * Moyen de scan de cet appareil (R-STK-50) : la caméra démarre dès l'ouverture du scan ; un lecteur de
 * code-barres scanne sans clic. Réglage gardé dans ce navigateur : chaque appareil a le sien.
 */
@Component({
  selector: 'app-scan-settings',
  imports: [MatRadioModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card" id="scan">
      <h2 class="widget-title">Scan des codes-barres</h2>
      <p class="muted">Propre à cet appareil : le téléphone et l’ordinateur ont chacun le leur.</p>
      <mat-radio-group
        aria-label="Moyen de scan"
        class="choices"
        [value]="device.mode()"
        (change)="device.set($any($event.value))"
      >
        @for (c of choices; track c.mode) {
          <mat-radio-button [value]="c.mode">
            <div>{{ c.label }}</div>
            <div class="muted">{{ c.hint }}</div>
          </mat-radio-button>
        }
      </mat-radio-group>
    </section>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .choices {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
  `,
})
export class ScanSettings {
  protected readonly device = inject(ScanDevice);
  protected readonly choices: { mode: ScanDeviceMode; label: string; hint: string }[] = [
    {
      mode: 'camera',
      label: 'Caméra',
      hint: 'Elle démarre dès qu’on ouvre le scan (HTTPS nécessaire).',
    },
    {
      mode: 'reader',
      label: 'Lecteur de code-barres (douchette)',
      hint: 'USB ou Bluetooth : il scanne directement, sans clic.',
    },
  ];
}
