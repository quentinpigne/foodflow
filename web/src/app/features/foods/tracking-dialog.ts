import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { firstValueFrom } from 'rxjs';

import { FoodDetail } from '../../api/models/food-detail';
import { FoodsService } from '../../api/services/foods.service';
import { StockService } from '../../api/services/stock.service';
import { errorMessage } from '../../core/http-errors';
import { TRACKING_LABELS } from '../../core/labels';
import { parseQuantity } from '../../core/quantity';

/** Change le mode de suivi d'un aliment qui a des lots, en les convertissant (R-STK-21). */
@Component({
  selector: 'app-tracking-dialog',
  imports: [FormsModule, MatButtonModule, MatDialogModule, MatFormFieldModule, MatInputModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Suivre en {{ target === 'PRESENCE' ? 'présence' : 'quantité' }}</h2>
    <mat-dialog-content>
      @if (target === 'PRESENCE') {
        <p>
          Les lots de {{ food.name }} passeront à l'état « disponible » ; leur quantité ne sera plus
          suivie.
        </p>
      } @else {
        <p>Indiquer la quantité de chaque lot de {{ food.name }} en stock.</p>
        @for (lot of lots.value() ?? []; track lot.id) {
          <mat-form-field class="full-width">
            <mat-label>{{ lot.name }} — {{ lot.location_name }}</mat-label>
            <input
              matInput
              inputmode="decimal"
              [ngModel]="entries()[lot.id] ?? ''"
              (ngModelChange)="set(lot.id, $event)"
            />
          </mat-form-field>
        }
      }
      @if (error()) {
        <p class="error">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button [disabled]="busy()" (click)="save()">{{ labels[target] }}</button>
    </mat-dialog-actions>
  `,
  styles: `
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class TrackingDialog {
  protected readonly food = inject<FoodDetail>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<TrackingDialog>);
  private readonly api = inject(FoodsService);
  private readonly stock = inject(StockService);

  protected readonly labels = TRACKING_LABELS;
  protected readonly target =
    this.food.effective.tracking_mode === 'QUANTITY' ? 'PRESENCE' : 'QUANTITY';
  protected readonly entries = signal<Record<string, string>>({});
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly lots = rxResource({
    stream: () => this.stock.listLots({ food_id: this.food.id }),
  });

  protected set(id: string, value: string): void {
    this.entries.update((e) => ({ ...e, [id]: value }));
  }

  protected async save(): Promise<void> {
    const measure = this.food.effective.stock_measure;
    const inventory = (this.lots.value() ?? []).map((l) => ({
      lot_id: l.id,
      amount: parseQuantity(this.entries()[l.id] ?? '', measure),
    }));
    if (this.target === 'QUANTITY' && inventory.some((e) => !e.amount)) {
      this.error.set('Indiquer une quantité valide pour chaque lot.');
      return;
    }
    this.busy.set(true);
    try {
      await firstValueFrom(
        this.api.changeTrackingMode({
          id: this.food.id,
          body: { mode: this.target, inventory: this.target === 'QUANTITY' ? inventory : [] },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
