import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { firstValueFrom } from 'rxjs';

import { FoodsService } from '../../api/services/foods.service';
import { errorMessage } from '../../core/http-errors';
import { ItemPicker, StockItem } from '../../shared/item-picker';
import { ProductDialog } from '../foods/product-dialog';

export interface AttachBarcodeData {
  barcode: string;
}

/**
 * Code inconnu lu au scan (R-STK-52) : choisir un aliment crée un produit avec ce code ; choisir un
 * produit existant lui donne ce code. Se ferme sur `true` quand le code est rattaché.
 */
@Component({
  selector: 'app-attach-barcode-dialog',
  imports: [ItemPicker, MatButtonModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Code {{ data.barcode }}</h2>
    <mat-dialog-content>
      <p class="muted">
        Choisir l’aliment de ce produit pour le créer, ou le produit existant auquel donner ce code.
      </p>
      <app-item-picker label="Aliment ou produit" (picked)="pick($event)" />
      @if (error()) {
        <p class="error">{{ error() }}</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
    </mat-dialog-actions>
  `,
  styles: `
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AttachBarcodeDialog {
  protected readonly data = inject<AttachBarcodeData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<AttachBarcodeDialog>);
  private readonly dialog = inject(MatDialog);
  private readonly api = inject(FoodsService);
  protected readonly error = signal<string | null>(null);

  protected async pick(item: StockItem): Promise<void> {
    this.error.set(null);
    if (item.kind === 'food') {
      const created = this.dialog.open(ProductDialog, {
        data: {
          foodId: item.food.id,
          measure: item.food.effective.stock_measure,
          barcode: this.data.barcode,
        },
        width: '30rem',
      });
      if (await firstValueFrom(created.afterClosed())) {
        this.ref.close(true);
      }
      return;
    }
    const p = item.product;
    if (p.barcode && p.barcode !== this.data.barcode) {
      this.error.set(`${p.name} a déjà le code ${p.barcode} : un produit n’a qu’un code-barres.`);
      return;
    }
    try {
      await firstValueFrom(
        this.api.updateProduct({
          id: p.id,
          body: {
            food_id: p.food_id ?? null,
            group_id: p.group_id ?? null,
            name: p.name,
            brand: p.brand ?? null,
            barcode: this.data.barcode,
            stock_measure: p.group_id ? p.stock_measure : null,
            package_amount: p.package_amount ?? null,
            package_units: p.package_units ?? null,
            nutrient_basis: p.nutrient_basis,
          },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    }
  }
}
