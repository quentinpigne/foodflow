import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatChipsModule } from '@angular/material/chips';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { RouterLink } from '@angular/router';

import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { LotRow } from '../../shared/lot-row';
import { LocationsStore } from '../../shared/stores';
import { AddLot } from './add-lot';
import { fold } from '../../core/text';

/** Stock (STK-04) : lots par emplacement, triés par date limite effective. */
@Component({
  selector: 'app-stock',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSlideToggleModule,
    LotRow,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Stock</h1>
        <a mat-button routerLink="/stock/inventaire"><mat-icon>checklist</mat-icon>Inventaire</a>
        <a mat-button routerLink="/stock/scan"><mat-icon>barcode_scanner</mat-icon>Ranger</a>
        <button mat-flat-button (click)="addLot.open()"><mat-icon>add</mat-icon>Ajouter</button>
      </div>

      <div class="filters">
        <mat-chip-listbox
          aria-label="Emplacement"
          [value]="location()"
          (change)="location.set($event.value ?? null)"
        >
          <mat-chip-option [value]="null">Tout</mat-chip-option>
          @for (l of locations.list(); track l.id) {
            <mat-chip-option [value]="l.id">{{ l.name }} ({{ l.lot_count }})</mat-chip-option>
          }
        </mat-chip-listbox>
        <mat-form-field class="search">
          <mat-icon matPrefix>search</mat-icon>
          <input
            matInput
            placeholder="Filtrer"
            [ngModel]="filter()"
            (ngModelChange)="filter.set($event)"
          />
        </mat-form-field>
        <mat-slide-toggle [ngModel]="depleted()" (ngModelChange)="depleted.set($event)"
          >Épuisés</mat-slide-toggle
        >
      </div>

      @if (lots.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (visible().length) {
        <div class="lot-list">
          @for (lot of visible(); track lot.id) {
            <app-lot-row [lot]="lot" [showLocation]="!location()" />
          }
        </div>
      } @else if (lots.hasValue()) {
        <div class="empty">Aucun lot.</div>
      }
    </div>
  `,
  styles: `
    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem 1rem;
      margin-bottom: 1rem;
    }
    .search {
      width: 14rem;
    }
  `,
})
export class StockPage {
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);
  protected readonly locations = inject(LocationsStore);
  protected readonly addLot = inject(AddLot);

  protected readonly location = signal<string | null>(null);
  protected readonly filter = signal('');
  protected readonly depleted = signal(false);

  protected readonly lots = rxResource({
    params: () => ({
      v: this.version.value(),
      location: this.location(),
      depleted: this.depleted(),
    }),
    stream: ({ params }) =>
      this.api.listLots({
        location_id: params.location ?? undefined,
        include_depleted: params.depleted,
      }),
  });

  protected readonly visible = computed(() => {
    const lots = this.lots.hasValue() ? this.lots.value() : [];
    const words = fold(this.filter()).split(/\s+/).filter(Boolean);
    return words.length
      ? lots.filter((l) => {
          const text = fold(`${l.name} ${l.brand ?? ''} ${l.food_name ?? ''}`);
          return words.every((w) => text.includes(w));
        })
      : lots;
  });
}
