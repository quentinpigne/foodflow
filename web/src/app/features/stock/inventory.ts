import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatChipsModule } from '@angular/material/chips';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { InventoryEntry } from '../../api/models/inventory-entry';
import { Lot } from '../../api/models/lot';
import { PresenceState } from '../../api/models/presence-state';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { PRESENCE_LABELS } from '../../core/labels';
import { formatQuantity, parseQuantity, quantityInput } from '../../core/quantity';
import { LocationsStore } from '../../shared/stores';
import { AddLot } from './add-lot';

/** Inventaire d'un emplacement (STK-05, R-STK-40, R-STK-41) : la quantité constatée de chaque lot. */
@Component({
  selector: 'app-inventory',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/stock" aria-label="Retour au stock"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Inventaire</h1>
        <a mat-button routerLink="/stock/inventaire/scan"
          ><mat-icon>barcode_scanner</mat-icon>Par scan</a
        >
      </div>
      <p class="muted">
        Pour chaque lot, indiquer ce qu'il y a vraiment. L'écart avec le calcul est conservé ; un
        lot absent se met à zéro. Un lot trouvé s'ajoute au stock.
      </p>
      <mat-chip-listbox
        aria-label="Emplacement"
        [value]="location()"
        (change)="location.set($event.value)"
      >
        @for (l of locations.list(); track l.id) {
          <mat-chip-option [value]="l.id">{{ l.name }}</mat-chip-option>
        }
      </mat-chip-listbox>

      @if (lots.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (lots.hasValue()) {
        <div class="rows">
          @for (lot of lots.value(); track lot.id) {
            <div class="row">
              <div class="label">
                <div>{{ lot.name }}</div>
                <div class="muted">
                  Calculé :
                  {{
                    lot.tracking_mode === 'QUANTITY'
                      ? formatQuantity(lot.quantity, lot.measure)
                      : presenceLabels[lot.presence ?? 'AVAILABLE']
                  }}
                </div>
              </div>
              @if (lot.tracking_mode === 'QUANTITY') {
                <mat-form-field class="field">
                  <mat-label>Constaté</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="observed()[lot.id]"
                    (ngModelChange)="set(lot.id, $event)"
                  />
                  @if (invalid(lot)) {
                    <mat-hint class="error">Invalide</mat-hint>
                  }
                </mat-form-field>
              } @else {
                <mat-form-field class="field">
                  <mat-label>État</mat-label>
                  <mat-select [ngModel]="observed()[lot.id]" (ngModelChange)="set(lot.id, $event)">
                    @for (p of presenceStates; track p) {
                      <mat-option [value]="p">{{ presenceLabels[p] }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
              }
            </div>
          } @empty {
            <div class="empty">Aucun lot à cet emplacement.</div>
          }
        </div>
        <div class="actions">
          <button mat-stroked-button (click)="addLot.open()">
            <mat-icon>add</mat-icon>Lot trouvé
          </button>
          <button
            mat-flat-button
            [disabled]="busy() || !lots.value().length || hasInvalid()"
            (click)="save()"
          >
            Enregistrer l’inventaire
          </button>
        </div>
      }
    </div>
  `,
  styles: `
    mat-chip-listbox {
      display: block;
      margin: 1rem 0;
    }
    .rows {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      margin-bottom: 1rem;
    }
    .row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
      padding: 0.25rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }
    .field {
      width: 10rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class InventoryPage {
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  protected readonly locations = inject(LocationsStore);
  protected readonly addLot = inject(AddLot);

  protected readonly presenceStates: PresenceState[] = ['AVAILABLE', 'LOW', 'OUT'];
  protected readonly presenceLabels = PRESENCE_LABELS;
  protected readonly formatQuantity = formatQuantity;

  protected readonly location = signal<string | null>(null);
  protected readonly observed = signal<Record<string, string>>({});
  protected readonly busy = signal(false);

  protected readonly lots = rxResource({
    params: () => ({ location: this.location(), v: this.version.value() }),
    stream: ({ params }) => this.api.listLots({ location_id: params.location ?? undefined }),
  });

  protected readonly hasInvalid = computed(() =>
    (this.lots.hasValue() ? this.lots.value() : []).some((l) => this.invalid(l)),
  );

  constructor() {
    // Premier emplacement par défaut ; la saisie part des quantités calculées.
    effect(() => {
      const first = this.locations.list()[0];
      if (!this.location() && first) {
        this.location.set(first.id);
      }
    });
    effect(() => {
      const lots = this.lots.hasValue() ? this.lots.value() : [];
      this.observed.set(
        Object.fromEntries(
          lots.map((l) => [
            l.id,
            l.tracking_mode === 'QUANTITY'
              ? quantityInput(l.quantity ?? 0, l.measure)
              : (l.presence ?? 'AVAILABLE'),
          ]),
        ),
      );
    });
  }

  protected set(id: string, value: string): void {
    this.observed.update((current) => ({ ...current, [id]: value }));
  }

  protected invalid(lot: Lot): boolean {
    return (
      lot.tracking_mode === 'QUANTITY' &&
      parseQuantity(this.observed()[lot.id] ?? '', lot.measure) === null
    );
  }

  protected async save(): Promise<void> {
    const lots = this.lots.hasValue() ? this.lots.value() : [];
    const entries: InventoryEntry[] = lots.map((l) =>
      l.tracking_mode === 'QUANTITY'
        ? { lot_id: l.id, amount: parseQuantity(this.observed()[l.id], l.measure) }
        : { lot_id: l.id, presence: this.observed()[l.id] as PresenceState },
    );
    this.busy.set(true);
    try {
      await firstValueFrom(this.api.recordInventory({ body: { entries } }));
      this.version.bump();
      this.snack.open('Inventaire enregistré', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }
}
