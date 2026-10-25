import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { RouterLink } from '@angular/router';
import { Observable, firstValueFrom } from 'rxjs';

import { Lot } from '../../api/models/lot';
import { StockMovement } from '../../api/models/stock-movement';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { todayIso } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import {
  EXPIRY_KIND_LABELS,
  MOVEMENT_LABELS,
  PRESENCE_LABELS,
  STATUS_LABELS,
  WASTE_LABELS,
} from '../../core/labels';
import { Layout } from '../../core/layout';
import { formatQuantity } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { statusClass } from '../../shared/lot-row';
import { MoneyPipe } from '../../shared/money.pipe';
import { QuantityPipe } from '../../shared/quantity.pipe';
import { LocationsStore } from '../../shared/stores';
import { LotAction, LotActionDialog } from './lot-actions';

/** Fiche d'un lot (STK-03) : état, gestes et journal des mouvements, avec annulation (R-STK-11). */
@Component({
  selector: 'app-lot-page',
  imports: [
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatMenuModule,
    MatProgressBarModule,
    FrDatePipe,
    MoneyPipe,
    QuantityPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/stock" aria-label="Retour au stock"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>{{ detail.value()?.name ?? 'Lot' }}</h1>
      </div>
      @if (detail.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (detail.hasValue()) {
        @let lot = detail.value();
        <section class="card summary">
          <div class="quantity">
            @if (lot.tracking_mode === 'QUANTITY') {
              {{ lot.quantity | quantity: lot.measure }}
            } @else {
              {{ presenceLabels[lot.presence ?? 'AVAILABLE'] }}
            }
          </div>
          <div class="chips">
            @if (lot.depleted) {
              <span class="status">Épuisé</span>
            }
            @if (lot.needs_review) {
              <span class="status review">À vérifier</span>
            }
            @if (lot.effective_expiry; as e) {
              <span [class]="'status ' + cls()">
                {{ statusLabels[lot.status] }} · limite {{ e.date | frDate }}
              </span>
            }
          </div>
          <dl>
            @if (lot.brand || lot.food_name) {
              <dt>Produit</dt>
              <dd>
                {{ lot.brand }}
                @if (lot.food_id) {
                  · <a [routerLink]="['/aliments', lot.food_id]">{{ lot.food_name }}</a>
                } @else {
                  {{ lot.food_name }}
                }
              </dd>
            } @else if (lot.food_id) {
              <dt>Aliment</dt>
              <dd><a [routerLink]="['/aliments', lot.food_id]">Voir la fiche</a></dd>
            }
            <dt>Emplacement</dt>
            <dd>{{ lot.location_name }}</dd>
            <dt>Péremption</dt>
            <dd>
              {{ expiryLabels[lot.expiry_kind] }}
              @if (lot.expiry_date) {
                · {{ lot.expiry_date | frDate }}
              }
              @if (lot.expiry_missing) {
                · <span class="status review">à compléter</span>
              }
            </dd>
            <dt>Entré le</dt>
            <dd>{{ lot.stocked_on | frDate }}</dd>
            @if (lot.opened_on) {
              <dt>Ouvert le</dt>
              <dd>{{ lot.opened_on | frDate }}</dd>
            }
            @if (lot.frozen_on) {
              <dt>Congelé le</dt>
              <dd>{{ lot.frozen_on | frDate }}</dd>
            }
            @if (lot.thawed_on) {
              <dt>Décongelé le</dt>
              <dd>{{ lot.thawed_on | frDate }}</dd>
            }
            @if (lot.checked_until) {
              <dt>Jugé bon jusqu'au</dt>
              <dd>{{ lot.checked_until | frDate }}</dd>
            }
            @if ((lot.unit_price ?? null) !== null) {
              <dt>Prix</dt>
              <dd>{{ lot.unit_price | money }}</dd>
            }
            @if (lot.note) {
              <dt>Note</dt>
              <dd>{{ lot.note }}</dd>
            }
          </dl>
          @if (!lot.depleted) {
            <div class="actions">
              <button mat-flat-button (click)="act(lot, 'consume')">
                <mat-icon>restaurant</mat-icon>Consommer
              </button>
              @if (lot.status === 'EXPIRED' || lot.status === 'PAST_BEST_BEFORE') {
                <button mat-stroked-button (click)="act(lot, 'check')">
                  <mat-icon>thumb_up</mat-icon>Encore bon
                </button>
              }
              <button mat-stroked-button (click)="act(lot, 'waste')">
                <mat-icon>delete</mat-icon>Jeter
              </button>
              <button mat-stroked-button (click)="act(lot, 'move')">
                <mat-icon>swap_horiz</mat-icon>Déplacer
              </button>
              <button mat-icon-button [matMenuTriggerFor]="more" aria-label="Autres actions">
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #more>
                @if (!lot.opened_on) {
                  <button mat-menu-item (click)="open(lot)">
                    <mat-icon>lock_open</mat-icon>Ouvert aujourd'hui
                  </button>
                }
                <button mat-menu-item (click)="act(lot, 'adjust')">
                  <mat-icon>tune</mat-icon>Corriger
                </button>
                <button mat-menu-item (click)="act(lot, 'edit')">
                  <mat-icon>edit</mat-icon>Modifier
                </button>
              </mat-menu>
            </div>
          }
          @if (lot.depleted && lot.needs_review) {
            <div class="actions">
              <button mat-stroked-button (click)="confirmEmpty(lot)">
                <mat-icon>check</mat-icon>Confirmer : il n'en reste plus
              </button>
            </div>
          }
        </section>

        <h2 class="journal-title">Journal</h2>
        <div class="journal">
          @for (m of lot.movements; track m.id) {
            <div class="movement" [class.cancelled]="m.reversed || m.reverses_id">
              <div>
                <div>
                  <strong>{{ movementLabels[m.kind] }}</strong>
                  @if (m.reverses_id) {
                    (annulation)
                  }
                  @if (m.waste_reason) {
                    · {{ wasteLabels[m.waste_reason] }}
                  }
                  @if (m.kind === 'MOVE') {
                    · {{ locationName(m.from_location_id) }} → {{ locationName(m.to_location_id) }}
                  }
                  @if (m.checked_until) {
                    · jusqu'au {{ m.checked_until | frDate }}
                  }
                </div>
                <div class="muted">
                  {{ m.date | frDate }}
                  @if (m.shortfall) {
                    · {{ m.kind === 'INVENTORY' ? 'écart' : 'manquait' }}
                    {{ amount(m.shortfall, lot) }}
                  }
                </div>
              </div>
              <div class="right">
                @if ((m.amount ?? null) !== null) {
                  <span class="amount" [class.negative]="(m.amount ?? 0) < 0">{{
                    signed(m, lot)
                  }}</span>
                } @else if (m.presence) {
                  <span>{{ presenceLabels[m.presence] }}</span>
                }
                @if (!m.reversed && !m.reverses_id) {
                  <button mat-icon-button aria-label="Annuler ce mouvement" (click)="reverse(m)">
                    <mat-icon>undo</mat-icon>
                  </button>
                }
              </div>
            </div>
          }
        </div>
      }
    </div>
  `,
  styles: `
    .summary {
      display: flex;
      flex-direction: column;
      gap: 0.75rem;
      margin-bottom: 1.5rem;
    }
    .quantity {
      font: var(--mat-sys-headline-medium);
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    dl {
      display: grid;
      grid-template-columns: max-content 1fr;
      gap: 0.25rem 1rem;
      margin: 0;
    }
    dt {
      color: var(--mat-sys-on-surface-variant);
    }
    dd {
      margin: 0;
    }
    .journal-title {
      font: var(--mat-sys-title-medium);
    }
    .journal {
      display: flex;
      flex-direction: column;
    }
    .movement {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      padding: 0.5rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }
    .movement.cancelled {
      opacity: 0.55;
    }
    .right {
      display: flex;
      align-items: center;
      gap: 0.25rem;
    }
  `,
})
export class LotPage {
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly layout = inject(Layout);
  private readonly locations = inject(LocationsStore);

  readonly id = input.required<string>();

  protected readonly statusLabels = STATUS_LABELS;
  protected readonly expiryLabels = EXPIRY_KIND_LABELS;
  protected readonly movementLabels = MOVEMENT_LABELS;
  protected readonly presenceLabels = PRESENCE_LABELS;
  protected readonly wasteLabels = WASTE_LABELS;

  protected readonly detail = rxResource({
    params: () => ({ id: this.id(), v: this.version.value() }),
    stream: ({ params }) => this.api.getLot({ id: params.id }),
  });

  protected readonly cls = computed(() =>
    this.detail.hasValue() ? statusClass(this.detail.value()) : '',
  );

  protected locationName(id: string | null | undefined): string {
    return this.locations.list().find((l) => l.id === id)?.name ?? '?';
  }

  protected amount(raw: number, lot: Lot): string {
    return formatQuantity(raw, lot.measure);
  }

  protected signed(m: StockMovement, lot: Lot): string {
    const text = formatQuantity(m.amount, lot.measure);
    return m.kind === 'INVENTORY' ? `= ${text}` : (m.amount ?? 0) > 0 ? `+${text}` : text;
  }

  protected async act(lot: Lot, action: LotAction): Promise<void> {
    const ref = this.dialog.open(LotActionDialog, {
      data: { lot, action },
      ...(this.layout.mobile() ? { panelClass: 'fullscreen-dialog' } : { width: '30rem' }),
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async open(lot: Lot): Promise<void> {
    await this.run(
      this.api.openLot({ id: lot.id, body: { date: todayIso() } }),
      'Lot marqué ouvert',
    );
  }

  /** Lot épuisé à vérifier : un inventaire à zéro confirme l'écart (R-STK-40). */
  protected async confirmEmpty(lot: Lot): Promise<void> {
    const entry = {
      lot_id: lot.id,
      amount: lot.tracking_mode === 'QUANTITY' ? 0 : null,
      presence: lot.tracking_mode === 'PRESENCE' ? ('OUT' as const) : null,
    };
    await this.run(this.api.recordInventory({ body: { entries: [entry] } }), 'Écart confirmé');
  }

  protected async reverse(m: StockMovement): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler ce mouvement ?',
      message: m.batch_id
        ? 'Ce mouvement fait partie d’une opération groupée : toute l’opération sera annulée.'
        : 'Un mouvement inverse sera enregistré ; le journal garde la trace des deux.',
      action: 'Annuler le mouvement',
    });
    if (ok) {
      await this.run(this.api.reverseMovement({ id: m.id }), 'Mouvement annulé');
    }
  }

  private async run(request: Observable<unknown>, message: string): Promise<void> {
    try {
      await firstValueFrom(request);
      this.version.bump();
      this.snack.open(message, undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }
}
