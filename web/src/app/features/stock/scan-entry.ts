import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
  untracked,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatChipsModule } from '@angular/material/chips';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { BarcodeLookup } from '../../api/models/barcode-lookup';
import { ExpiryKind } from '../../api/models/expiry-kind';
import { LotInput } from '../../api/models/lot-input';
import { FoodsService } from '../../api/services/foods.service';
import { StockService } from '../../api/services/stock.service';
import { ScanOutcome, scanFeedback } from '../../core/barcode';
import { DataVersion } from '../../core/data-version';
import {
  addDays,
  addMonths,
  formatShortDate,
  fromIsoDate,
  toIsoDate,
  todayIso,
} from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { EXPIRY_KIND_LABELS } from '../../core/labels';
import { parseQuantity } from '../../core/quantity';
import { BarcodeScanner } from '../../shared/barcode-scanner';
import { LocationsStore } from '../../shared/stores';
import { AttachBarcodeDialog } from './attach-barcode';
import { EntryLine, addAgain, addScan, newLine } from './scan';

const DRAFT_KEY = 'foodflow.scan-entry';

/**
 * Ranger les courses en scannant (STK-12) : chaque lecture ajoute un conditionnement (R-STK-51), la revue
 * propose les lots (R-STK-53), une seule validation les crée, annulable d'un geste (R-STK-54).
 */
@Component({
  selector: 'app-scan-entry',
  imports: [
    BarcodeScanner,
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatChipsModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/stock" aria-label="Retour au stock"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Ranger les courses</h1>
      </div>
      <p class="muted">
        Scanner les produits à la suite : rien n’entre en stock avant la validation. Les dates
        peuvent rester à compléter.
      </p>

      <div class="default-location">
        <span class="muted">Emplacement par défaut</span>
        <mat-chip-listbox
          aria-label="Emplacement par défaut"
          [value]="defaultLocation()"
          (change)="defaultLocation.set($event.value ?? defaultLocation())"
        >
          @for (l of locations.list(); track l.id) {
            <mat-chip-option [value]="l.id">{{ l.name }}</mat-chip-option>
          }
        </mat-chip-listbox>
      </div>

      <app-barcode-scanner (scanned)="onScan($event)" />

      @if (last(); as l) {
        <p class="last" [class]="l.outcome">{{ l.text }}</p>
      }

      <div class="lines">
        @for (line of lines(); track line.barcode; let i = $index) {
          <div class="line" [class.unknown]="!line.product">
            <div class="head">
              <div class="name">
                @if (line.product; as p) {
                  <div class="title">
                    {{ p.name }}
                    @if (p.brand) {
                      <span class="muted">· {{ p.brand }}</span>
                    }
                  </div>
                  <div class="muted">{{ p.attached_to }}</div>
                } @else if (line.draft; as d) {
                  <div class="title">
                    {{ d.name ?? 'Produit sans nom' }}
                    @if (d.brand) {
                      <span class="muted">· {{ d.brand }}</span>
                    }
                  </div>
                  <div class="muted">
                    Nouveau produit, d’après Open Food Facts
                    @if (d.proposed_name) {
                      · {{ d.proposed_name }}
                    }
                  </div>
                } @else {
                  <div class="title">Code inconnu</div>
                  <div class="muted">
                    {{ line.barcode }}
                    @if (line.offStatus === 'NOT_FOUND') {
                      · absent d’Open Food Facts
                    } @else if (line.offStatus === 'UNAVAILABLE') {
                      · Open Food Facts injoignable
                    }
                  </div>
                }
              </div>
              <div class="count">
                <button
                  mat-icon-button
                  aria-label="Un de moins"
                  [disabled]="line.count <= 1"
                  (click)="update(i, { count: line.count - 1 })"
                >
                  <mat-icon>remove</mat-icon>
                </button>
                <span class="n">× {{ line.count }}</span>
                <button
                  mat-icon-button
                  aria-label="Un de plus"
                  (click)="update(i, { count: line.count + 1 })"
                >
                  <mat-icon>add</mat-icon>
                </button>
                <button mat-icon-button aria-label="Retirer la ligne" (click)="remove(i)">
                  <mat-icon>delete</mat-icon>
                </button>
              </div>
            </div>

            @if (!line.product) {
              @if (line.draft) {
                <button mat-flat-button (click)="attach(i, line)">
                  <mat-icon>check</mat-icon>Vérifier et créer le produit
                </button>
              } @else {
                <button mat-stroked-button (click)="attach(i, line)">
                  <mat-icon>link</mat-icon>Rattacher à un aliment ou un produit
                </button>
              }
            } @else {
              <div class="fields">
                <mat-form-field subscriptSizing="dynamic" class="location">
                  <mat-label>Emplacement</mat-label>
                  <mat-select
                    [ngModel]="line.location"
                    (ngModelChange)="update(i, { location: $event })"
                  >
                    @for (l of locations.list(); track l.id) {
                      <mat-option [value]="l.id">{{ l.name }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
                @if (line.tracking === 'QUANTITY') {
                  <mat-form-field subscriptSizing="dynamic" class="quantity">
                    <mat-label>Quantité d’un lot</mat-label>
                    <input
                      matInput
                      inputmode="decimal"
                      [ngModel]="line.quantity"
                      (ngModelChange)="update(i, { quantity: $event })"
                    />
                    @if (amountOf(line) === null) {
                      <mat-hint class="error">À saisir (« 500 g »)</mat-hint>
                    }
                  </mat-form-field>
                }
                <mat-form-field subscriptSizing="dynamic" class="kind">
                  <mat-label>Péremption</mat-label>
                  <mat-select
                    [ngModel]="line.expiryKind"
                    (ngModelChange)="update(i, { expiryKind: $event })"
                  >
                    @for (k of expiryKinds; track k) {
                      <mat-option [value]="k">{{ expiryLabels[k] }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
              </div>
              @if (line.expiryKind !== 'NONE') {
                <div class="expiry">
                  <span [class.missing]="!line.expiryDate">
                    {{ line.expiryDate ? formatShortDate(line.expiryDate) : 'Date à compléter' }}
                  </span>
                  <button mat-button (click)="update(i, { expiryDate: addDays(today, 3) })">
                    +3 j
                  </button>
                  <button mat-button (click)="update(i, { expiryDate: addDays(today, 7) })">
                    +1 sem.
                  </button>
                  <button mat-button (click)="update(i, { expiryDate: addMonths(today, 1) })">
                    +1 mois
                  </button>
                  <input
                    class="hidden-input"
                    [matDatepicker]="picker"
                    [ngModel]="line.expiryDate ? fromIsoDate(line.expiryDate) : null"
                    (ngModelChange)="update(i, { expiryDate: $event ? toIsoDate($event) : null })"
                  />
                  <mat-datepicker-toggle [for]="picker" aria-label="Choisir la date" />
                  <mat-datepicker #picker />
                  @if (line.expiryDate) {
                    <button
                      mat-icon-button
                      aria-label="Date à compléter"
                      (click)="update(i, { expiryDate: null })"
                    >
                      <mat-icon>close</mat-icon>
                    </button>
                  }
                </div>
              }
            }
          </div>
        } @empty {
          <div class="empty">Aucun produit scanné.</div>
        }
      </div>

      @if (lines().length) {
        <div class="actions">
          <button mat-button (click)="clear()">Tout effacer</button>
          <button mat-flat-button [disabled]="busy() || !ready()" (click)="save()">
            Mettre en stock ({{ lotCount() }} {{ lotCount() > 1 ? 'lots' : 'lot' }})
          </button>
        </div>
        @if (!ready()) {
          <p class="muted right">
            Rattacher les codes inconnus et saisir les quantités manquantes pour valider.
          </p>
        }
      }
    </div>
  `,
  styles: `
    .default-location {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
      margin: 0.5rem 0 1rem;
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
    .lines {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      margin: 1rem 0;
    }
    .line {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
      padding: 0.75rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-surface-container);
    }
    .line.unknown {
      outline: 1px dashed var(--mat-sys-tertiary);
    }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem;
    }
    .title {
      font: var(--mat-sys-title-small);
    }
    .count {
      display: flex;
      align-items: center;
      flex-shrink: 0;
    }
    .n {
      min-width: 2.5rem;
      text-align: center;
      font: var(--mat-sys-title-small);
    }
    .fields {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .location,
    .kind {
      flex: 1 1 10rem;
    }
    .quantity {
      flex: 0 1 9rem;
    }
    .expiry {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.25rem;
    }
    .expiry > span {
      margin-right: 0.5rem;
    }
    .missing {
      color: var(--mat-sys-tertiary);
    }
    .hidden-input {
      width: 0;
      border: 0;
      padding: 0;
      opacity: 0;
    }
    .error {
      color: var(--mat-sys-error);
    }
    .right {
      text-align: right;
    }
  `,
})
export class ScanEntryPage {
  private readonly foods = inject(FoodsService);
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);
  private readonly snack = inject(MatSnackBar);
  private readonly dialog = inject(MatDialog);
  protected readonly locations = inject(LocationsStore);

  protected readonly expiryKinds: ExpiryKind[] = ['USE_BY', 'BEST_BEFORE', 'NONE'];
  protected readonly expiryLabels = EXPIRY_KIND_LABELS;
  protected readonly today = todayIso();
  protected readonly addDays = addDays;
  protected readonly addMonths = addMonths;
  protected readonly formatShortDate = formatShortDate;
  protected readonly fromIsoDate = fromIsoDate;
  protected readonly toIsoDate = toIsoDate;

  protected readonly defaultLocation = signal<string | null>(null);
  protected readonly lines = signal<EntryLine[]>(loadDraft());
  protected readonly last = signal<{ outcome: ScanOutcome; text: string } | null>(null);
  protected readonly busy = signal(false);

  protected readonly lotCount = computed(() => this.lines().reduce((sum, l) => sum + l.count, 0));
  protected readonly ready = computed(() =>
    this.lines().every(
      (l) =>
        !!l.product && !!l.location && (l.tracking === 'PRESENCE' || this.amountOf(l) !== null),
    ),
  );

  constructor() {
    effect(() => {
      const first = this.locations.list()[0];
      if (!untracked(this.defaultLocation) && first) {
        this.defaultLocation.set(first.id);
      }
    });
    // Brouillon gardé sur l'appareil : une page rechargée en plein rangement ne perd rien.
    effect(() => saveDraft(this.lines()));
  }

  protected async onScan(code: string): Promise<void> {
    const again = addAgain(this.lines(), code);
    if (again) {
      this.lines.set(again);
      const line = again[0];
      scanFeedback(line.product ? 'known' : 'unknown');
      this.last.set({
        outcome: line.product ? 'known' : 'unknown',
        text: `${line.product?.name ?? line.draft?.name ?? 'Code ' + code} × ${line.count}`,
      });
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
    this.lines.update((lines) => addScan(lines, lookup, this.defaultLocation()));
    const line = this.lines()[0];
    if (lookup.product) {
      scanFeedback('known');
      this.last.set({ outcome: 'known', text: `${lookup.product.name} × ${line.count}` });
    } else {
      scanFeedback('unknown');
      this.last.set({
        outcome: 'unknown',
        text: lookup.draft?.name
          ? `${lookup.draft.name} : nouveau produit, à valider`
          : `Code ${lookup.barcode} inconnu : à rattacher`,
      });
    }
  }

  protected update(index: number, change: Partial<EntryLine>): void {
    this.lines.update((lines) => lines.map((l, i) => (i === index ? { ...l, ...change } : l)));
  }

  protected remove(index: number): void {
    this.lines.update((lines) => lines.filter((_, i) => i !== index));
  }

  protected clear(): void {
    this.lines.set([]);
    this.last.set(null);
  }

  /** Rattache un code inconnu, puis relit le code pour proposer les lots du produit (R-STK-52). */
  protected async attach(index: number, line: EntryLine): Promise<void> {
    const ref = this.dialog.open(AttachBarcodeDialog, {
      data: { barcode: line.barcode, draft: line.draft, offStatus: line.offStatus },
      width: '32rem',
    });
    if (!(await firstValueFrom(ref.afterClosed()))) {
      return;
    }
    const lookup = await firstValueFrom(this.foods.lookupBarcode({ code: line.barcode }));
    const fresh = newLine(lookup, line.location ?? this.defaultLocation());
    this.update(index, { ...fresh, count: line.count });
    this.version.bump();
  }

  protected amountOf(line: EntryLine): number | null {
    if (!line.product) {
      return null;
    }
    const amount = parseQuantity(line.quantity, line.product.stock_measure);
    return amount && amount > 0 ? amount : null;
  }

  protected async save(): Promise<void> {
    const lines: LotInput[] = this.lines().map((l) => ({
      product_id: l.product?.id ?? null,
      food_id: null,
      location_id: l.location ?? '',
      amount: l.tracking === 'QUANTITY' ? this.amountOf(l) : null,
      expiry_kind: l.expiryKind,
      expiry_date: l.expiryKind !== 'NONE' ? l.expiryDate : null,
      expiry_missing: l.expiryKind !== 'NONE' && !l.expiryDate,
      count: l.count,
    }));
    this.busy.set(true);
    try {
      const result = await firstValueFrom(this.api.createLotBatch({ body: { lines } }));
      const count = result.ids.length;
      this.clear();
      this.version.bump();
      this.snack
        .open(`${count} ${count > 1 ? 'lots ajoutés' : 'lot ajouté'} au stock`, 'Annuler', {
          duration: 8000,
        })
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
      this.snack.open('Mise en stock annulée', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }
}

function loadDraft(): EntryLine[] {
  try {
    const raw = localStorage.getItem(DRAFT_KEY);
    return raw ? (JSON.parse(raw) as EntryLine[]) : [];
  } catch {
    return [];
  }
}

function saveDraft(lines: EntryLine[]): void {
  try {
    if (lines.length) {
      localStorage.setItem(DRAFT_KEY, JSON.stringify(lines));
    } else {
      localStorage.removeItem(DRAFT_KEY);
    }
  } catch {
    // Stockage indisponible : le brouillon n'est simplement pas gardé.
  }
}
