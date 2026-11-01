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
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { BarcodeLookup } from '../../api/models/barcode-lookup';
import { InventoryEntry } from '../../api/models/inventory-entry';
import { Lot } from '../../api/models/lot';
import { LotInput } from '../../api/models/lot-input';
import { FoodsService } from '../../api/services/foods.service';
import { StockService } from '../../api/services/stock.service';
import { ScanOutcome, scanFeedback } from '../../core/barcode';
import { DataVersion } from '../../core/data-version';
import { formatShortDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { formatQuantity, parseQuantity, quantityInput } from '../../core/quantity';
import { BarcodeScanner } from '../../shared/barcode-scanner';
import { LocationsStore } from '../../shared/stores';
import { AttachBarcodeDialog } from './attach-barcode';
import { EMPTY_INVENTORY, InventoryScan, assignScan } from './scan';

/**
 * Inventaire d'un emplacement par scan (STK-13, R-STK-55) : chaque lecture constate un lot, ou compte un
 * lot trouvé ; les lots non scannés restent inchangés.
 */
@Component({
  selector: 'app-scan-inventory',
  imports: [
    BarcodeScanner,
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/stock/inventaire" aria-label="Retour à l’inventaire"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Inventaire par scan</h1>
      </div>
      <p class="muted">
        Scanner chaque paquet présent. Un paquet entamé se corrige à la main. Les lots non scannés
        ne changent pas.
      </p>
      <mat-chip-listbox
        aria-label="Emplacement"
        [value]="location()"
        (change)="changeLocation($event.value)"
      >
        @for (l of locations.list(); track l.id) {
          <mat-chip-option [value]="l.id">{{ l.name }}</mat-chip-option>
        }
      </mat-chip-listbox>

      <app-barcode-scanner (scanned)="onScan($event)" />
      @if (last(); as l) {
        <p class="last" [class]="l.outcome">
          {{ l.text }}
          @if (l.outcome === 'unknown' && l.lookup; as lookup) {
            <button mat-button (click)="attach(lookup)">
              {{ lookup.draft ? 'Vérifier et créer' : 'Rattacher' }}
            </button>
          }
        </p>
      }
      @if (lots.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }

      @if (observedLots().length) {
        <h2>Constatés</h2>
        <div class="rows">
          @for (lot of observedLots(); track lot.id) {
            <div class="row">
              <div class="label">
                <div>{{ lot.name }}</div>
                <div class="muted">
                  {{ lot.expiry_date ? formatShortDate(lot.expiry_date) : 'Sans date' }} · calculé
                  {{
                    lot.tracking_mode === 'QUANTITY'
                      ? formatQuantity(lot.quantity, lot.measure)
                      : 'en présence'
                  }}
                </div>
              </div>
              @if (lot.tracking_mode === 'QUANTITY') {
                <mat-form-field subscriptSizing="dynamic" class="field">
                  <mat-label>Constaté</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="amounts()[lot.id]"
                    (ngModelChange)="setAmount(lot.id, $event)"
                  />
                </mat-form-field>
              } @else {
                <span class="muted">Disponible</span>
              }
              <button mat-icon-button aria-label="Retirer" (click)="unobserve(lot.id)">
                <mat-icon>close</mat-icon>
              </button>
            </div>
          }
        </div>
      }

      @if (foundLines().length) {
        <h2>Trouvés</h2>
        <div class="rows">
          @for (f of foundLines(); track f.lookup.barcode) {
            <div class="row">
              <div class="label">
                <div>{{ f.lookup.product?.name }} × {{ f.count }}</div>
                <div class="muted">
                  Absent du stock à cet emplacement : lot ajouté, date à compléter
                </div>
              </div>
              @if (f.lookup.tracking_mode !== 'PRESENCE' && f.lookup.product; as p) {
                <mat-form-field subscriptSizing="dynamic" class="field">
                  <mat-label>Quantité d’un lot</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="foundAmounts()[f.productId]"
                    (ngModelChange)="setFoundAmount(f.productId, $event)"
                  />
                </mat-form-field>
              }
              <button mat-icon-button aria-label="Un de moins" (click)="unfind(f.productId)">
                <mat-icon>remove</mat-icon>
              </button>
            </div>
          }
        </div>
      }

      @if (lots.hasValue()) {
        <p class="muted">
          {{ unscanned().length }}
          {{ unscanned().length > 1 ? 'lots non scannés' : 'lot non scanné' }}, inchangés{{
            unscanned().length ? ' : ' + unscannedNames() : ''
          }}
        </p>
      }

      <div class="actions">
        <button mat-button [disabled]="!dirty()" (click)="reset()">Recommencer</button>
        <button mat-flat-button [disabled]="busy() || !dirty() || invalid()" (click)="save()">
          Enregistrer l’inventaire
        </button>
      </div>
    </div>
  `,
  styles: `
    mat-chip-listbox {
      display: block;
      margin: 1rem 0;
    }
    h2 {
      font: var(--mat-sys-title-medium);
      margin: 1rem 0 0.5rem;
    }
    .last {
      font: var(--mat-sys-title-small);
      margin: 0.75rem 0 0;
    }
    .last.unknown {
      color: var(--mat-sys-tertiary);
    }
    .last.invalid {
      color: var(--mat-sys-error);
    }
    .rows {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
    .row {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      padding: 0.25rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }
    .label {
      flex: 1;
    }
    .field {
      width: 8rem;
    }
  `,
})
export class ScanInventoryPage {
  private readonly foods = inject(FoodsService);
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  private readonly dialog = inject(MatDialog);
  protected readonly locations = inject(LocationsStore);

  protected readonly formatQuantity = formatQuantity;
  protected readonly formatShortDate = formatShortDate;

  protected readonly location = signal<string | null>(null);
  protected readonly scan = signal<InventoryScan>(EMPTY_INVENTORY);
  /** Quantité constatée saisie, par lot ; la quantité calculée par défaut. */
  protected readonly amounts = signal<Record<string, string>>({});
  /** Quantité d'un lot trouvé, par produit ; le conditionnement par défaut. */
  protected readonly foundAmounts = signal<Record<string, string>>({});
  /** Produits lus pendant la séance, pour les lots trouvés. */
  private readonly products = signal<Record<string, BarcodeLookup>>({});
  protected readonly last = signal<{
    outcome: ScanOutcome;
    text: string;
    lookup?: BarcodeLookup;
  } | null>(null);
  protected readonly busy = signal(false);

  protected readonly lots = rxResource({
    params: () => ({ location: this.location(), v: this.version.value() }),
    stream: ({ params }) => this.api.listLots({ location_id: params.location ?? undefined }),
  });
  private readonly lotList = computed(() => (this.lots.hasValue() ? this.lots.value() : []));

  protected readonly observedLots = computed(() => {
    const byId = new Map(this.lotList().map((l) => [l.id, l]));
    return this.scan()
      .observed.map((id) => byId.get(id))
      .filter((l): l is Lot => !!l)
      .reverse();
  });
  protected readonly foundLines = computed(() =>
    Object.entries(this.scan().found)
      .filter(([, count]) => count > 0)
      .map(([productId, count]) => ({ productId, count, lookup: this.products()[productId] })),
  );
  protected readonly unscanned = computed(() =>
    this.lotList().filter((l) => !this.scan().observed.includes(l.id)),
  );
  protected readonly unscannedNames = computed(() =>
    this.unscanned()
      .map((l) => l.name)
      .join(', '),
  );
  protected readonly dirty = computed(
    () => this.scan().observed.length > 0 || this.foundLines().length > 0,
  );
  protected readonly invalid = computed(
    () =>
      this.observedLots().some(
        (l) =>
          l.tracking_mode === 'QUANTITY' &&
          parseQuantity(this.amounts()[l.id] ?? '', l.measure) === null,
      ) ||
      this.foundLines().some(
        (f) => f.lookup.tracking_mode !== 'PRESENCE' && !this.foundAmount(f.productId),
      ),
  );

  constructor() {
    effect(() => {
      const first = this.locations.list()[0];
      if (!this.location() && first) {
        this.location.set(first.id);
      }
    });
  }

  protected changeLocation(id: string | null): void {
    if (id && id !== this.location()) {
      this.location.set(id);
      this.reset();
    }
  }

  protected reset(): void {
    this.scan.set(EMPTY_INVENTORY);
    this.amounts.set({});
    this.foundAmounts.set({});
    this.last.set(null);
  }

  protected async onScan(code: string): Promise<void> {
    if (!this.lots.hasValue()) {
      return;
    }
    let lookup: BarcodeLookup;
    try {
      lookup = await firstValueFrom(this.foods.lookupBarcode({ code }));
    } catch {
      scanFeedback('invalid');
      this.last.set({ outcome: 'invalid', text: `Code ${code} illisible ou erroné` });
      return;
    }
    const product = lookup.product;
    if (!product) {
      scanFeedback('unknown');
      this.last.set({
        outcome: 'unknown',
        text: lookup.draft?.name
          ? `${lookup.draft.name} : nouveau produit, d’après Open Food Facts`
          : `Code ${lookup.barcode} inconnu`,
        lookup,
      });
      return;
    }
    this.record(lookup);
  }

  private record(lookup: BarcodeLookup): void {
    const product = lookup.product;
    if (!product) {
      return;
    }
    this.products.update((p) => ({ ...p, [product.id]: lookup }));
    const before = this.scan();
    const after = assignScan(before, product.id, this.lotList());
    this.scan.set(after);
    const lotId = after.observed.find((id) => !before.observed.includes(id));
    const lot = this.lotList().find((l) => l.id === lotId);
    if (lot) {
      if (lot.tracking_mode === 'QUANTITY') {
        this.setAmount(lot.id, quantityInput(lot.quantity ?? 0, lot.measure));
      }
      this.last.set({ outcome: 'known', text: `${product.name} constaté` });
    } else {
      if (product.package_amount && this.foundAmounts()[product.id] === undefined) {
        this.setFoundAmount(
          product.id,
          quantityInput(product.package_amount, product.stock_measure),
        );
      }
      this.last.set({ outcome: 'known', text: `${product.name} trouvé en plus` });
    }
    scanFeedback('known');
  }

  /** Code inconnu : brouillon d'Open Food Facts à valider, ou rattachement à la main (R-STK-52). */
  protected async attach(lookup: BarcodeLookup): Promise<void> {
    const ref = this.dialog.open(AttachBarcodeDialog, {
      data: { barcode: lookup.barcode, draft: lookup.draft, offStatus: lookup.off_status },
      width: '32rem',
    });
    if (!(await firstValueFrom(ref.afterClosed()))) {
      return;
    }
    this.version.bump();
    this.record(await firstValueFrom(this.foods.lookupBarcode({ code: lookup.barcode })));
  }

  protected setAmount(id: string, value: string): void {
    this.amounts.update((a) => ({ ...a, [id]: value }));
  }

  protected setFoundAmount(productId: string, value: string): void {
    this.foundAmounts.update((a) => ({ ...a, [productId]: value }));
  }

  /** Quantité saisie d'un lot trouvé, si elle est valide et positive. */
  private foundAmount(productId: string): number | null {
    const product = this.products()[productId]?.product;
    if (!product) {
      return null;
    }
    const amount = parseQuantity(this.foundAmounts()[productId] ?? '', product.stock_measure);
    return amount && amount > 0 ? amount : null;
  }

  protected unobserve(id: string): void {
    this.scan.update((s) => ({ ...s, observed: s.observed.filter((o) => o !== id) }));
  }

  protected unfind(productId: string): void {
    this.scan.update((s) => ({
      ...s,
      found: { ...s.found, [productId]: Math.max(0, (s.found[productId] ?? 0) - 1) },
    }));
  }

  protected async save(): Promise<void> {
    const location = this.location();
    if (!location) {
      return;
    }
    const entries: InventoryEntry[] = this.observedLots().map((l) =>
      l.tracking_mode === 'QUANTITY'
        ? { lot_id: l.id, amount: parseQuantity(this.amounts()[l.id] ?? '', l.measure) }
        : { lot_id: l.id, presence: 'AVAILABLE' },
    );
    const found: LotInput[] = this.foundLines().map(({ productId, lookup, count }) => {
      const kind = lookup.expiry_kind ?? 'USE_BY';
      return {
        product_id: lookup.product?.id ?? null,
        food_id: null,
        location_id: location,
        amount: lookup.tracking_mode === 'PRESENCE' ? null : this.foundAmount(productId),
        expiry_kind: kind,
        expiry_missing: kind !== 'NONE',
        count,
      };
    });
    this.busy.set(true);
    try {
      const result = await firstValueFrom(this.api.recordInventory({ body: { entries, found } }));
      this.reset();
      this.version.bump();
      this.snack
        .open('Inventaire enregistré', 'Annuler', { duration: 8000 })
        .onAction()
        .subscribe(() => void this.undo(result.batch_id));
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }

  private async undo(batch: string): Promise<void> {
    try {
      await firstValueFrom(this.api.reverseBatch({ id: batch }));
      this.version.bump();
      this.snack.open('Inventaire annulé', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }
}
