import { ChangeDetectionStrategy, Component, input, model } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';

import { EffectiveSettings } from '../api/models/effective-settings';
import { Measure } from '../api/models/measure';
import { Settings } from '../api/models/settings';
import { TrackingMode } from '../api/models/tracking-mode';
import { MEASURE_LABELS, TRACKING_LABELS } from '../core/labels';

type DayField =
  | 'shelf_life_days'
  | 'opened_shelf_life_days'
  | 'frozen_shelf_life_days'
  | 'thawed_shelf_life_days';

/**
 * Réglages hérités d'une famille ou d'un aliment (R-ALI-2) : vide = hérité, la valeur héritée est
 * affichée en indication.
 */
@Component({
  selector: 'app-settings-editor',
  imports: [FormsModule, MatFormFieldModule, MatInputModule, MatSelectModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let s = settings();
    <div class="form-grid">
      <mat-form-field>
        <mat-label>Mesure de stock</mat-label>
        <mat-select
          [ngModel]="s.stock_measure ?? null"
          (ngModelChange)="patch({ stock_measure: $event })"
          [disabled]="lockMeasure()"
        >
          <mat-option [value]="null"
            >Héritée{{
              inherited() ? ' (' + measureLabels[inherited()!.stock_measure] + ')' : ''
            }}</mat-option
          >
          @for (m of measures; track m) {
            <mat-option [value]="m">{{ measureLabels[m] }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field>
        <mat-label>Suivi du stock</mat-label>
        <mat-select
          [ngModel]="s.tracking_mode ?? null"
          (ngModelChange)="patch({ tracking_mode: $event })"
          [disabled]="lockTracking()"
        >
          <mat-option [value]="null"
            >Hérité{{
              inherited() ? ' (' + trackingLabels[inherited()!.tracking_mode] + ')' : ''
            }}</mat-option
          >
          @for (t of trackings; track t) {
            <mat-option [value]="t">{{ trackingLabels[t] }}</mat-option>
          }
        </mat-select>
        @if (lockTracking()) {
          <mat-hint>Des lots sont en stock : changer depuis « Mode de suivi ».</mat-hint>
        }
      </mat-form-field>
      @for (f of dayFields; track f.key) {
        <mat-form-field>
          <mat-label>{{ f.label }}</mat-label>
          <input
            matInput
            type="number"
            min="0"
            [ngModel]="s[f.key] ?? null"
            (ngModelChange)="patchDays(f.key, $event)"
            [placeholder]="hint(f.key)"
          />
          <span matTextSuffix>j</span>
        </mat-form-field>
      }
    </div>
  `,
})
export class SettingsEditor {
  readonly settings = model.required<Settings>();
  /** Valeurs héritées du parent, pour les indications. */
  readonly inherited = input<EffectiveSettings | null>(null);
  readonly lockTracking = input(false);
  readonly lockMeasure = input(false);

  protected readonly measures: Measure[] = ['MASS', 'VOLUME', 'COUNT'];
  protected readonly trackings: TrackingMode[] = ['QUANTITY', 'PRESENCE'];
  protected readonly measureLabels = MEASURE_LABELS;
  protected readonly trackingLabels = TRACKING_LABELS;
  protected readonly dayFields: { key: DayField; label: string }[] = [
    { key: 'shelf_life_days', label: 'Conservation sans date' },
    { key: 'opened_shelf_life_days', label: 'Après ouverture' },
    { key: 'frozen_shelf_life_days', label: 'Au congélateur' },
    { key: 'thawed_shelf_life_days', label: 'Après décongélation' },
  ];

  protected hint(key: DayField): string {
    const value = this.inherited()?.[key];
    return value != null ? `hérité : ${value}` : 'hérité : aucune';
  }

  protected patch(change: Partial<Settings>): void {
    this.settings.update((s) => ({ ...s, ...change }));
  }

  protected patchDays(key: DayField, value: number | string | null): void {
    const days = value === null || value === '' ? null : Math.max(0, Math.trunc(Number(value)));
    this.patch({ [key]: Number.isNaN(days) ? null : days });
  }
}
