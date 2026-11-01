import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { firstValueFrom } from 'rxjs';

import { Measure } from '../../api/models/measure';
import { OffStatus } from '../../api/models/off-status';
import { ProductDraft } from '../../api/models/product-draft';
import { FoodsService } from '../../api/services/foods.service';
import { errorMessage } from '../../core/http-errors';
import { formatQuantity, parseQuantity, quantityInput } from '../../core/quantity';
import { ItemPicker, StockItem, itemName } from '../../shared/item-picker';
import { ProductDialog } from '../foods/product-dialog';

export interface AttachBarcodeData {
  barcode: string;
  /** Brouillon tiré d'Open Food Facts (R-ALI-60). */
  draft?: ProductDraft | null;
  offStatus?: OffStatus | null;
}

/** Rattachement choisi pour le brouillon : un aliment, ou la famille proposée. */
type Attachment =
  | { kind: 'food'; id: string; name: string; measure: Measure }
  | { kind: 'group'; id: string; name: string; measure: Measure };

const OFF_MESSAGES: Partial<Record<OffStatus, string>> = {
  NOT_FOUND: 'Produit absent d’Open Food Facts.',
  UNAVAILABLE: 'Open Food Facts injoignable : le produit se décrit à la main.',
};

/**
 * Code inconnu lu au scan (R-STK-52). Avec un brouillon d'Open Food Facts (R-ALI-60) : nom, marque et
 * conditionnement préremplis, rattachement proposé ; le produit n'est créé qu'en validant (ADR-0005).
 * Sans brouillon : choisir un aliment pour créer le produit, ou un produit existant qui reçoit ce code.
 * Se ferme sur `true` quand le code est rattaché.
 */
@Component({
  selector: 'app-attach-barcode-dialog',
  imports: [
    FormsModule,
    ItemPicker,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Code {{ data.barcode }}</h2>
    <mat-dialog-content>
      @if (draft; as d) {
        <p class="muted">
          Trouvé dans Open Food Facts{{
            d.nutrients.length ? ', avec ses valeurs nutritionnelles' : ''
          }}. Rien n’est créé avant validation.
        </p>
        <form id="draft" class="form" (ngSubmit)="createFromDraft()">
          <mat-form-field>
            <mat-label>Nom</mat-label>
            <input
              matInput
              name="name"
              required
              maxlength="200"
              [ngModel]="name()"
              (ngModelChange)="name.set($event)"
            />
          </mat-form-field>
          <mat-form-field>
            <mat-label>Marque</mat-label>
            <input matInput name="brand" [ngModel]="brand()" (ngModelChange)="brand.set($event)" />
          </mat-form-field>

          @if (attachment(); as a) {
            <div class="picked">
              <div>
                <div class="muted">{{ a.kind === 'food' ? 'Aliment' : 'Famille' }}</div>
                <div class="title">{{ a.name }}</div>
              </div>
              <button mat-button type="button" (click)="attachment.set(null)">Changer</button>
            </div>
          } @else {
            <app-item-picker
              label="Aliment de ce produit"
              [foodsOnly]="true"
              (picked)="pickFood($event)"
            />
          }

          @if (attachment(); as a) {
            <mat-form-field>
              <mat-label>Conditionnement</mat-label>
              <input matInput name="pack" [ngModel]="pack()" (ngModelChange)="pack.set($event)" />
              @if (packHint(); as hint) {
                <mat-hint>{{ hint }}</mat-hint>
              }
            </mat-form-field>
          }
          @if (error()) {
            <p class="error">{{ error() }}</p>
          }
        </form>
      } @else {
        <p class="muted">
          @if (data.offStatus && offMessages[data.offStatus]; as message) {
            {{ message }}
          }
          Choisir l’aliment de ce produit pour le créer, ou le produit existant auquel donner ce
          code.
        </p>
        <app-item-picker label="Aliment ou produit" (picked)="pick($event)" />
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      @if (draft) {
        <button
          mat-flat-button
          form="draft"
          type="submit"
          [disabled]="busy() || !attachment() || !name().trim()"
        >
          Créer le produit
        </button>
      }
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(26rem, 100%);
    }
    .picked {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0.5rem 0.75rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-surface-container);
    }
    .title {
      font: var(--mat-sys-title-small);
    }
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

  protected readonly offMessages = OFF_MESSAGES;
  protected readonly draft = this.data.draft ?? null;
  protected readonly name = signal(this.draft?.name ?? '');
  protected readonly brand = signal(this.draft?.brand ?? '');
  protected readonly attachment = signal<Attachment | null>(null);
  protected readonly pack = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  /** Le conditionnement lu ne vaut que dans la mesure de l'aliment (R-ALI-42). */
  protected readonly packHint = computed(() => {
    const d = this.draft;
    const a = this.attachment();
    if (!d?.package_amount || !d.package_measure || !a || d.package_measure === a.measure) {
      return null;
    }
    return `Open Food Facts indique ${formatQuantity(d.package_amount, d.package_measure)} : à convertir.`;
  });

  constructor() {
    void this.proposeAttachment();
  }

  /** Rattachement proposé par le brouillon : catégorie mémorisée ou nom (R-ALI-60, R-ALI-62). */
  private async proposeAttachment(): Promise<void> {
    const d = this.draft;
    if (d?.proposed_food_id) {
      const food = await firstValueFrom(this.api.getFood({ id: d.proposed_food_id }));
      this.attach({
        kind: 'food',
        id: food.id,
        name: food.name,
        measure: food.effective.stock_measure,
      });
    } else if (d?.proposed_group_id) {
      this.attach({
        kind: 'group',
        id: d.proposed_group_id,
        name: d.proposed_name ?? '',
        measure: d.package_measure ?? 'MASS',
      });
    }
  }

  private attach(a: Attachment): void {
    this.attachment.set(a);
    const d = this.draft;
    this.pack.set(
      d?.package_amount && d.package_measure === a.measure
        ? quantityInput(d.package_amount, a.measure)
        : '',
    );
  }

  protected pickFood(item: StockItem): void {
    if (item.kind === 'food') {
      this.attach({
        kind: 'food',
        id: item.food.id,
        name: itemName(item),
        measure: item.food.effective.stock_measure,
      });
    }
  }

  protected async createFromDraft(): Promise<void> {
    const d = this.draft;
    const a = this.attachment();
    if (!d || !a) {
      return;
    }
    const pack = this.pack().trim() ? parseQuantity(this.pack(), a.measure) : null;
    if (this.pack().trim() && !pack) {
      this.error.set('Conditionnement invalide.');
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.createProduct({
          body: {
            food_id: a.kind === 'food' ? a.id : null,
            group_id: a.kind === 'group' ? a.id : null,
            stock_measure: a.kind === 'group' ? a.measure : null,
            name: this.name().trim(),
            brand: this.brand().trim() || null,
            barcode: this.data.barcode,
            package_amount: pack,
            package_units: pack ? (d.package_units ?? null) : null,
            nutrient_basis: d.nutrient_basis,
            off_payload: d.off_payload,
          },
        }),
      );
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

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
