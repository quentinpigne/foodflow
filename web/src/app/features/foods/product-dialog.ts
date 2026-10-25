import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { Observable, firstValueFrom } from 'rxjs';

import { Measure } from '../../api/models/measure';
import { Product } from '../../api/models/product';
import { FoodsService } from '../../api/services/foods.service';
import { errorMessage } from '../../core/http-errors';
import { UNITS, defaultUnit, parseQuantity, quantityInput } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';

export interface ProductDialogData {
  foodId: string;
  measure: Measure;
  product?: Product;
}

/** Produit du commerce d'un aliment (ALI-05) : marque, code-barres, conditionnement (R-ALI-41, R-ALI-42). */
@Component({
  selector: 'app-product-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ product ? 'Produit' : 'Nouveau produit' }}</h2>
    <mat-dialog-content>
      <form id="product" class="form" (ngSubmit)="save()">
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input
            matInput
            name="name"
            [ngModel]="name()"
            (ngModelChange)="name.set($event)"
            required
            maxlength="200"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Marque</mat-label>
          <input
            matInput
            name="brand"
            [ngModel]="brand()"
            (ngModelChange)="brand.set($event)"
            maxlength="120"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Code-barres</mat-label>
          <input
            matInput
            name="barcode"
            inputmode="numeric"
            [ngModel]="barcode()"
            (ngModelChange)="barcode.set($event)"
          />
          <mat-hint>EAN-8, EAN-13 ou UPC-A</mat-hint>
        </mat-form-field>
        <div class="row">
          <mat-form-field class="grow">
            <mat-label>Conditionnement</mat-label>
            <input
              matInput
              name="package"
              inputmode="decimal"
              [ngModel]="pack()"
              (ngModelChange)="pack.set($event)"
            />
            <mat-hint>Ex. « 1 L », « 500 g », « 6 »</mat-hint>
          </mat-form-field>
          <mat-form-field class="unit">
            <mat-label>Unités</mat-label>
            <input
              matInput
              name="units"
              type="number"
              min="1"
              [ngModel]="units()"
              (ngModelChange)="units.set($event)"
            />
            <mat-hint>Lot de…</mat-hint>
          </mat-form-field>
        </div>
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      @if (product) {
        <button mat-button class="danger" (click)="remove()">Supprimer</button>
      }
      <span class="spacer"></span>
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button form="product" type="submit" [disabled]="busy() || !name().trim()">
        Enregistrer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .row {
      display: flex;
      gap: var(--field-gap);
    }
    .grow {
      flex: 1;
    }
    .unit {
      width: 7rem;
    }
    .error,
    .danger {
      color: var(--mat-sys-error);
    }
    .spacer {
      flex: 1;
    }
  `,
})
export class ProductDialog {
  private readonly data = inject<ProductDialogData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<ProductDialog>);
  private readonly api = inject(FoodsService);
  private readonly dialog = inject(MatDialog);

  protected readonly product = this.data.product;
  protected readonly name = signal(this.product?.name ?? '');
  protected readonly brand = signal(this.product?.brand ?? '');
  protected readonly barcode = signal(this.product?.barcode ?? '');
  protected readonly pack = signal(
    this.product?.package_amount
      ? quantityInput(this.product.package_amount, this.data.measure)
      : '',
  );
  protected readonly units = signal<number | null>(this.product?.package_units ?? null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected async save(): Promise<void> {
    const measure = this.data.measure;
    const pack = this.pack().trim()
      ? parseQuantity(this.pack(), measure, defaultUnit(measure))
      : null;
    if (this.pack().trim() && !pack) {
      this.error.set(
        `Conditionnement invalide (${UNITS[measure].map((u) => u.symbol).join(', ')}).`,
      );
      return;
    }
    const body = {
      food_id: this.data.foodId,
      name: this.name(),
      brand: this.brand() || null,
      barcode: this.barcode() || null,
      package_amount: pack,
      package_units: this.units() || null,
    };
    await this.run(
      this.product
        ? this.api.updateProduct({ id: this.product.id, body })
        : this.api.createProduct({ body }),
    );
  }

  protected async remove(): Promise<void> {
    if (!this.product) return;
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le produit ?',
      message: 'Un produit qui a des lots en stock ne peut pas être supprimé.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) await this.run(this.api.deleteProduct({ id: this.product.id }));
  }

  private async run(request: Observable<unknown>): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(request);
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
