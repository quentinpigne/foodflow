import { NgTemplateOutlet } from '@angular/common';
import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { Observable, firstValueFrom } from 'rxjs';

import { ExpiryKind } from '../../api/models/expiry-kind';
import { Lot } from '../../api/models/lot';
import { PresenceState } from '../../api/models/presence-state';
import { WasteReason } from '../../api/models/waste-reason';
import { StockService } from '../../api/services/stock.service';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { EXPIRY_KIND_LABELS, PRESENCE_LABELS, WASTE_LABELS } from '../../core/labels';
import { formatMoney, parseMoney } from '../../core/money';
import { UNITS, defaultUnit, formatQuantity, parseQuantity } from '../../core/quantity';
import { LocationsStore } from '../../shared/stores';

export type LotAction = 'consume' | 'waste' | 'adjust' | 'move' | 'check' | 'edit';

export interface LotActionData {
  lot: Lot;
  action: LotAction;
}

const TITLES: Record<LotAction, string> = {
  consume: 'Consommer',
  waste: 'Jeter',
  adjust: 'Corriger',
  move: 'Déplacer',
  check: 'Encore bon',
  edit: 'Modifier le lot',
};

/** Les gestes sur un lot (STK-03, STK-06, STK-07) : une boîte, des champs selon le geste. */
@Component({
  selector: 'app-lot-action-dialog',
  imports: [
    FormsModule,
    NgTemplateOutlet,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatDatepickerModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ title }} — {{ lot.name }}</h2>
    <mat-dialog-content>
      <form id="lot-action" class="form" (ngSubmit)="save()">
        @if (action !== 'edit') {
          <mat-form-field>
            <mat-label>Date</mat-label>
            <input
              matInput
              name="date"
              [matDatepicker]="datePicker"
              [max]="today"
              [ngModel]="date()"
              (ngModelChange)="date.set($event)"
            />
            <mat-datepicker-toggle matIconSuffix [for]="datePicker" />
            <mat-datepicker #datePicker />
          </mat-form-field>
        }

        @switch (action) {
          @case ('consume') {
            @if (quantityMode) {
              <ng-container [ngTemplateOutlet]="quantityField" />
              <p class="muted">En stock : {{ current }}</p>
            } @else {
              <mat-form-field>
                <mat-label>État après</mat-label>
                <mat-select
                  name="presence"
                  [ngModel]="presence()"
                  (ngModelChange)="presence.set($event)"
                >
                  <mat-option [value]="null">Inchangé</mat-option>
                  @for (p of presenceStates; track p) {
                    <mat-option [value]="p">{{ presenceLabels[p] }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            }
          }
          @case ('waste') {
            <mat-form-field>
              <mat-label>Motif</mat-label>
              <mat-select name="reason" [ngModel]="reason()" (ngModelChange)="reason.set($event)">
                @for (r of reasons; track r) {
                  <mat-option [value]="r">{{ wasteLabels[r] }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            @if (quantityMode) {
              <mat-checkbox name="all" [ngModel]="all()" (ngModelChange)="all.set($event)"
                >Tout le lot ({{ current }})</mat-checkbox
              >
              @if (!all()) {
                <ng-container [ngTemplateOutlet]="quantityField" />
              }
            }
          }
          @case ('adjust') {
            @if (quantityMode) {
              <mat-button-toggle-group
                name="sign"
                [ngModel]="sign()"
                (ngModelChange)="sign.set($event)"
              >
                <mat-button-toggle [value]="1">Ajouter</mat-button-toggle>
                <mat-button-toggle [value]="-1">Retirer</mat-button-toggle>
              </mat-button-toggle-group>
              <ng-container [ngTemplateOutlet]="quantityField" />
              <p class="muted">En stock : {{ current }}</p>
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
          }
          @case ('move') {
            <mat-form-field>
              <mat-label>Vers</mat-label>
              <mat-select name="to" [ngModel]="to()" (ngModelChange)="to.set($event)" required>
                @for (l of destinations(); track l.id) {
                  <mat-option [value]="l.id">{{ l.name }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            @if (quantityMode) {
              <mat-checkbox name="all" [ngModel]="all()" (ngModelChange)="all.set($event)"
                >Tout le lot ({{ current }})</mat-checkbox
              >
              @if (!all()) {
                <ng-container [ngTemplateOutlet]="quantityField" />
              }
            }
          }
          @case ('check') {
            <p>La date est dépassée, mais le produit est encore bon : jusqu'à quand le garder ?</p>
            <mat-form-field>
              <mat-label>Nouvelle date limite</mat-label>
              <input
                matInput
                name="until"
                [matDatepicker]="untilPicker"
                [ngModel]="until()"
                (ngModelChange)="until.set($event)"
              />
              <mat-datepicker-toggle matIconSuffix [for]="untilPicker" />
              <mat-datepicker #untilPicker />
            </mat-form-field>
          }
          @case ('edit') {
            <mat-form-field>
              <mat-label>Péremption</mat-label>
              <mat-select
                name="kind"
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
                  name="expiry"
                  [matDatepicker]="expiryPicker"
                  [ngModel]="expiryDate()"
                  (ngModelChange)="expiryDate.set($event)"
                />
                <mat-datepicker-toggle matIconSuffix [for]="expiryPicker" />
                <mat-datepicker #expiryPicker />
              </mat-form-field>
            }
            <mat-form-field>
              <mat-label>Prix</mat-label>
              <input
                matInput
                name="price"
                inputmode="decimal"
                [ngModel]="price()"
                (ngModelChange)="price.set($event)"
              />
              <span matTextSuffix>€</span>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Note</mat-label>
              <input matInput name="note" [ngModel]="note()" (ngModelChange)="note.set($event)" />
            </mat-form-field>
          }
        }

        <ng-template #quantityField>
          <div class="row">
            <mat-form-field class="grow">
              <mat-label>Quantité</mat-label>
              <input
                matInput
                name="quantity"
                inputmode="decimal"
                [ngModel]="quantity()"
                (ngModelChange)="quantity.set($event)"
              />
            </mat-form-field>
            <mat-form-field class="unit">
              <mat-label>Unité</mat-label>
              <mat-select name="unit" [ngModel]="unit()" (ngModelChange)="unit.set($event)">
                @for (u of units; track u.symbol) {
                  <mat-option [value]="u.symbol">{{ u.symbol }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>
        </ng-template>

        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button form="lot-action" type="submit" [disabled]="busy()">
        {{ title }}
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
      min-width: min(24rem, 100%);
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
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class LotActionDialog {
  private readonly data = inject<LotActionData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<LotActionDialog>);
  private readonly api = inject(StockService);
  private readonly locations = inject(LocationsStore);

  protected readonly lot = this.data.lot;
  protected readonly action = this.data.action;
  protected readonly title = TITLES[this.action];
  protected readonly today = new Date();
  protected readonly quantityMode = this.lot.tracking_mode === 'QUANTITY';
  protected readonly current = formatQuantity(this.lot.quantity, this.lot.measure);
  protected readonly units = UNITS[this.lot.measure];
  protected readonly presenceStates: PresenceState[] = ['AVAILABLE', 'LOW', 'OUT'];
  protected readonly presenceLabels = PRESENCE_LABELS;
  protected readonly reasons: WasteReason[] = ['EXPIRED', 'SPOILED', 'OTHER'];
  protected readonly wasteLabels = WASTE_LABELS;
  protected readonly expiryKinds: ExpiryKind[] = ['USE_BY', 'BEST_BEFORE', 'NONE'];
  protected readonly expiryLabels = EXPIRY_KIND_LABELS;

  protected readonly date = signal<Date>(new Date());
  protected readonly quantity = signal('');
  protected readonly unit = signal(defaultUnit(this.lot.measure).symbol);
  protected readonly presence = signal<PresenceState | null>(
    this.action === 'adjust' ? (this.lot.presence ?? 'AVAILABLE') : null,
  );
  protected readonly reason = signal<WasteReason>(this.lot.status === 'OK' ? 'SPOILED' : 'EXPIRED');
  protected readonly all = signal(true);
  protected readonly sign = signal(1);
  protected readonly to = signal<string | null>(null);
  protected readonly until = signal<Date>(new Date(Date.now() + 86_400_000));
  protected readonly expiryKind = signal<ExpiryKind>(this.lot.expiry_kind);
  protected readonly expiryDate = signal<Date | null>(
    this.lot.expiry_date ? fromIsoDate(this.lot.expiry_date) : null,
  );
  protected readonly price = signal(
    this.lot.unit_price != null ? formatMoney(this.lot.unit_price, { bare: true }) : '',
  );
  protected readonly note = signal(this.lot.note ?? '');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly destinations = computed(() =>
    this.locations.list().filter((l) => l.id !== this.lot.location_id),
  );

  private amount(): number | null {
    const unit = this.units.find((u) => u.symbol === this.unit());
    return parseQuantity(this.quantity(), this.lot.measure, unit);
  }

  private request(): Observable<unknown> | string {
    const id = this.lot.id;
    const date = toIsoDate(this.date());
    const amount = this.amount();
    switch (this.action) {
      case 'consume':
        if (this.quantityMode && !amount) return 'Quantité invalide.';
        return this.api.consumeLot({
          id,
          body: { date, amount: this.quantityMode ? amount : null, presence: this.presence() },
        });
      case 'waste':
        if (this.quantityMode && !this.all() && !amount) return 'Quantité invalide.';
        return this.api.wasteLot({
          id,
          body: {
            date,
            reason: this.reason(),
            amount: this.quantityMode && !this.all() ? amount : null,
          },
        });
      case 'adjust':
        if (this.quantityMode && !amount) return 'Quantité invalide.';
        return this.api.adjustLot({
          id,
          body: {
            date,
            // Une correction est signée ; le signe se choisit, pas l'arithmétique (quantity.ts).
            amount: this.quantityMode && amount ? (this.sign() < 0 ? -amount : amount) : null,
            presence: this.quantityMode ? null : this.presence(),
          },
        });
      case 'move': {
        const to = this.to();
        if (!to) return 'Choisir un emplacement.';
        if (this.quantityMode && !this.all() && !amount) return 'Quantité invalide.';
        return this.api.moveLot({
          id,
          body: {
            date,
            to_location_id: to,
            amount: this.quantityMode && !this.all() ? amount : null,
          },
        });
      }
      case 'check':
        return this.api.checkLot({ id, body: { date, until: toIsoDate(this.until()) } });
      case 'edit': {
        const price = this.price().trim() ? parseMoney(this.price()) : null;
        if (this.price().trim() && price === null) return 'Prix invalide.';
        const expiry = this.expiryDate();
        return this.api.updateLot({
          id,
          body: {
            expiry_kind: this.expiryKind(),
            expiry_date: this.expiryKind() !== 'NONE' && expiry ? toIsoDate(expiry) : null,
            expiry_missing: this.expiryKind() !== 'NONE' && !expiry,
            unit_price: price,
            note: this.note() || null,
          },
        });
      }
    }
  }

  protected async save(): Promise<void> {
    const request = this.request();
    if (typeof request === 'string') {
      this.error.set(request);
      return;
    }
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
