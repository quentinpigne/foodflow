import {
  ChangeDetectionStrategy,
  Component,
  Injectable,
  computed,
  effect,
  inject,
  signal,
  untracked,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ExpiryKind } from '../../api/models/expiry-kind';
import { Measure } from '../../api/models/measure';
import { PresenceState } from '../../api/models/presence-state';
import { TrackingMode } from '../../api/models/tracking-mode';
import { FoodsService } from '../../api/services/foods.service';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { EXPIRY_KIND_LABELS, PRESENCE_LABELS } from '../../core/labels';
import { Layout } from '../../core/layout';
import { parseMoney } from '../../core/money';
import { UNITS, bestUnit, defaultUnit, formatIn, parseQuantity } from '../../core/quantity';
import { BarcodeScanner } from '../../shared/barcode-scanner';
import { ItemPicker, StockItem, itemName } from '../../shared/item-picker';
import { scanFeedback } from '../../core/barcode';
import { AttachBarcodeDialog } from './attach-barcode';
import { GroupsStore, LocationsStore } from '../../shared/stores';

export interface AddLotData {
  item?: StockItem;
}

/** Ajouter au stock (STK-02) : un lot, ou plusieurs lots identiques (R-STK-6). */
@Component({
  selector: 'app-add-lot-dialog',
  imports: [
    BarcodeScanner,
    FormsModule,
    ItemPicker,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Ajouter au stock</h2>
    <mat-dialog-content>
      <form id="add-lot" class="form" (ngSubmit)="save()">
        @if (item(); as current) {
          <div class="picked">
            <div>
              <div class="name">{{ name() }}</div>
              <div class="muted">
                {{ tracking() === 'PRESENCE' ? 'Suivi en présence' : 'Suivi en quantité' }}
              </div>
            </div>
            <button mat-icon-button type="button" aria-label="Changer" (click)="item.set(null)">
              <mat-icon>close</mat-icon>
            </button>
          </div>
        } @else {
          <app-item-picker (picked)="choose($event)" />
          <div class="scan-row">
            <button mat-stroked-button type="button" (click)="scanning.set(!scanning())">
              <mat-icon>barcode_scanner</mat-icon>{{ scanning() ? 'Fermer le scan' : 'Scanner' }}
            </button>
            <a mat-button routerLink="/stock/scan" (click)="ref.close()"
              >Ranger plusieurs produits</a
            >
          </div>
          @if (scanning()) {
            <app-barcode-scanner (scanned)="scanPick($event)" />
          }
        }

        <mat-form-field>
          <mat-label>Emplacement</mat-label>
          <mat-select
            name="location"
            [ngModel]="location()"
            (ngModelChange)="location.set($event)"
            required
          >
            @for (l of locations.list(); track l.id) {
              <mat-option [value]="l.id">{{ l.name }}</mat-option>
            }
          </mat-select>
        </mat-form-field>

        @if (tracking() === 'QUANTITY') {
          <div class="row">
            <mat-form-field class="grow">
              <mat-label>Quantité par lot</mat-label>
              <input
                matInput
                name="quantity"
                [ngModel]="quantity()"
                (ngModelChange)="quantity.set($event)"
                inputmode="decimal"
                required
              />
              <span matTextSuffix>{{ unitSymbol() }}</span>
              @if (quantity() && parsedQuantity() === null) {
                <mat-hint class="error">Quantité invalide</mat-hint>
              }
            </mat-form-field>
            <mat-form-field class="unit">
              <mat-label>Unité</mat-label>
              <mat-select
                name="unit"
                [ngModel]="unitSymbol()"
                (ngModelChange)="unitSymbol.set($event)"
              >
                @for (u of units(); track u.symbol) {
                  <mat-option [value]="u.symbol">{{ u.symbol }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>
        } @else {
          <mat-form-field>
            <mat-label>État</mat-label>
            <mat-select
              name="presence"
              [ngModel]="presence()"
              (ngModelChange)="presence.set($event)"
            >
              @for (p of presenceStates; track p) {
                <mat-option [value]="p">{{ presenceLabels[p] }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        }

        <mat-form-field>
          <mat-label>Péremption</mat-label>
          <mat-select
            name="expiryKind"
            [ngModel]="expiryKind()"
            (ngModelChange)="expiryKind.set($event)"
          >
            @for (k of expiryKinds; track k) {
              <mat-option [value]="k">{{ expiryLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        @if (expiryKind() !== 'NONE') {
          <mat-form-field>
            <mat-label>Date</mat-label>
            <input
              matInput
              name="expiryDate"
              [matDatepicker]="picker"
              [ngModel]="expiryDate()"
              (ngModelChange)="expiryDate.set($event)"
              [disabled]="expiryMissing()"
            />
            <mat-datepicker-toggle matIconSuffix [for]="picker" />
            <mat-datepicker #picker />
          </mat-form-field>
          <mat-checkbox
            name="missing"
            [ngModel]="expiryMissing()"
            (ngModelChange)="expiryMissing.set($event)"
          >
            Date à compléter plus tard
          </mat-checkbox>
        }

        <div class="row">
          <mat-form-field class="grow">
            <mat-label>Nombre de lots</mat-label>
            <input
              matInput
              name="count"
              type="number"
              min="1"
              max="100"
              [ngModel]="count()"
              (ngModelChange)="count.set($event)"
            />
          </mat-form-field>
          <mat-form-field class="grow">
            <mat-label>Prix d'un lot</mat-label>
            <input
              matInput
              name="price"
              inputmode="decimal"
              [ngModel]="price()"
              (ngModelChange)="price.set($event)"
            />
            <span matTextSuffix>€</span>
          </mat-form-field>
        </div>
        <mat-form-field>
          <mat-label>Note</mat-label>
          <input matInput name="note" [ngModel]="note()" (ngModelChange)="note.set($event)" />
        </mat-form-field>
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button form="add-lot" type="submit" [disabled]="!canSave() || busy()">
        Ajouter
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(26rem, 100%);
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
    .scan-row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
    }
    .picked {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 0.5rem 0.75rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-surface-container);
    }
    .name {
      font: var(--mat-sys-title-small);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AddLotDialog {
  private readonly data = inject<AddLotData>(MAT_DIALOG_DATA);
  protected readonly ref = inject(MatDialogRef<AddLotDialog>);
  private readonly dialog = inject(MatDialog);
  private readonly api = inject(StockService);
  private readonly foods = inject(FoodsService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  private readonly groups = inject(GroupsStore);
  protected readonly locations = inject(LocationsStore);

  protected readonly expiryKinds: ExpiryKind[] = ['USE_BY', 'BEST_BEFORE', 'NONE'];
  protected readonly expiryLabels = EXPIRY_KIND_LABELS;
  protected readonly presenceStates: PresenceState[] = ['AVAILABLE', 'LOW', 'OUT'];
  protected readonly presenceLabels = PRESENCE_LABELS;

  protected readonly item = signal<StockItem | null>(this.data.item ?? null);
  protected readonly scanning = signal(false);
  protected readonly measure = signal<Measure>('MASS');
  protected readonly tracking = signal<TrackingMode>('QUANTITY');
  protected readonly location = signal<string | null>(null);
  protected readonly quantity = signal('');
  protected readonly unitSymbol = signal('g');
  protected readonly presence = signal<PresenceState>('AVAILABLE');
  protected readonly expiryKind = signal<ExpiryKind>('USE_BY');
  protected readonly expiryDate = signal<Date | null>(null);
  protected readonly expiryMissing = signal(false);
  protected readonly count = signal(1);
  protected readonly price = signal('');
  protected readonly note = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly name = computed(() => {
    const item = this.item();
    return item ? itemName(item) : '';
  });
  protected readonly units = computed(() => UNITS[this.measure()]);
  protected readonly parsedQuantity = computed(() => {
    const unit =
      this.units().find((u) => u.symbol === this.unitSymbol()) ?? defaultUnit(this.measure());
    return parseQuantity(this.quantity(), this.measure(), unit);
  });
  protected readonly canSave = computed(
    () =>
      !!this.item() &&
      !!this.location() &&
      (this.tracking() === 'PRESENCE' || (this.parsedQuantity() ?? 0) > 0) &&
      (this.expiryKind() === 'NONE' || this.expiryMissing() || !!this.expiryDate()),
  );

  constructor() {
    // Les emplacements peuvent arriver après l'ouverture : le premier est choisi dès qu'ils sont là.
    effect(() => {
      const first = this.locations.list()[0];
      if (!untracked(this.location) && first) {
        this.location.set(first.id);
      }
    });
    const item = this.item();
    if (item) {
      void this.choose(item);
    }
  }

  /** Choisit le produit par son code-barres (R-STK-50) ; un code inconnu se rattache d'abord (R-STK-52). */
  protected async scanPick(code: string): Promise<void> {
    this.error.set(null);
    try {
      let lookup = await firstValueFrom(this.foods.lookupBarcode({ code }));
      if (!lookup.product) {
        scanFeedback('unknown');
        const ref = this.dialog.open(AttachBarcodeDialog, {
          data: { barcode: lookup.barcode, draft: lookup.draft, offStatus: lookup.off_status },
          width: '32rem',
        });
        if (!(await firstValueFrom(ref.afterClosed()))) {
          return;
        }
        lookup = await firstValueFrom(this.foods.lookupBarcode({ code }));
      }
      if (lookup.product) {
        scanFeedback('known');
        this.scanning.set(false);
        await this.choose({ kind: 'product', product: lookup.product });
        if (lookup.location_id) {
          this.location.set(lookup.location_id);
        }
        if (lookup.expiry_kind) {
          this.expiryKind.set(lookup.expiry_kind);
        }
      }
    } catch {
      scanFeedback('invalid');
      this.error.set(`Code ${code} illisible ou erroné.`);
    }
  }

  /** Mesure et mode de suivi effectifs de l'élément choisi (R-ALI-2). */
  protected async choose(item: StockItem): Promise<void> {
    this.item.set(item);
    if (item.kind === 'food') {
      this.apply(item.food.effective.stock_measure, item.food.effective.tracking_mode);
      return;
    }
    const product = item.product;
    if (product.food_id) {
      const food = await firstValueFrom(this.foods.getFood({ id: product.food_id }));
      this.apply(product.stock_measure, food.effective.tracking_mode);
    } else {
      const group = this.groups.list().find((g) => g.id === product.group_id);
      this.apply(product.stock_measure, group?.effective.tracking_mode ?? 'QUANTITY');
    }
    // Un produit entre en stock par conditionnement : 1 L, 500 g (R-ALI-42).
    if (product.package_amount) {
      const unit = bestUnit(product.package_amount, product.stock_measure);
      this.unitSymbol.set(unit.symbol);
      this.quantity.set(formatIn(product.package_amount, unit));
    }
  }

  private apply(measure: Measure, tracking: TrackingMode): void {
    this.measure.set(measure);
    this.tracking.set(tracking);
    this.unitSymbol.set(defaultUnit(measure).symbol);
  }

  protected async save(): Promise<void> {
    const item = this.item();
    const location = this.location();
    if (!item || !location || !this.canSave()) {
      return;
    }
    const price = this.price().trim() ? parseMoney(this.price()) : null;
    if (this.price().trim() && price === null) {
      this.error.set('Prix invalide.');
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      const date = this.expiryDate();
      await firstValueFrom(
        this.api.createLots({
          body: {
            product_id: item.kind === 'product' ? item.product.id : null,
            food_id: item.kind === 'food' ? item.food.id : null,
            location_id: location,
            amount: this.tracking() === 'QUANTITY' ? this.parsedQuantity() : null,
            presence: this.tracking() === 'PRESENCE' ? this.presence() : null,
            expiry_kind: this.expiryKind(),
            expiry_date:
              this.expiryKind() !== 'NONE' && date && !this.expiryMissing()
                ? toIsoDate(date)
                : null,
            expiry_missing: this.expiryKind() !== 'NONE' && this.expiryMissing(),
            unit_price: price,
            count: Number(this.count()) || 1,
            note: this.note() || null,
          },
        }),
      );
      this.version.bump();
      this.snack.open(`${this.name()} ajouté au stock`, undefined, { duration: 3000 });
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}

/** Ouvre « Ajouter au stock » : plein écran sur mobile. */
@Injectable({ providedIn: 'root' })
export class AddLot {
  private readonly dialog = inject(MatDialog);
  private readonly layout = inject(Layout);

  open(data: AddLotData = {}): void {
    this.dialog.open(AddLotDialog, {
      data,
      autoFocus: 'first-tabbable',
      ...(this.layout.mobile() ? { panelClass: 'fullscreen-dialog' } : { width: '32rem' }),
    });
  }
}
