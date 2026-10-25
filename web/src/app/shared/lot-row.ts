import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { RouterLink } from '@angular/router';

import { Lot } from '../api/models/lot';
import { EXPIRY_KIND_SHORT, PRESENCE_LABELS, STATUS_LABELS } from '../core/labels';
import { formatQuantity } from '../core/quantity';
import { FrDatePipe } from './date.pipe';

/** Classe CSS de la pastille d'état d'un lot. */
export function statusClass(lot: Lot): string {
  switch (lot.status) {
    case 'SOON':
      return 'soon';
    case 'EXPIRED':
      return 'expired';
    case 'PAST_BEST_BEFORE':
      return 'past';
    default:
      return '';
  }
}

/** Une ligne de la liste du stock : contenu, emplacement, quantité et péremption (STK-04). */
@Component({
  selector: 'app-lot-row',
  imports: [RouterLink, FrDatePipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @let l = lot();
    <a class="lot-row" [routerLink]="['/stock', l.id]">
      <div>
        <div class="name">{{ l.name }}</div>
        <div class="meta">
          @if (l.brand) {
            {{ l.brand }} ·
          }
          @if (showLocation()) {
            {{ l.location_name }} ·
          }
          @if (l.opened_on) {
            ouvert ·
          }
          @if (l.frozen_on && !l.thawed_on) {
            congelé ·
          }
          @if (l.thawed_on) {
            décongelé ·
          }
          {{ quantityText() }}
        </div>
      </div>
      <div class="right">
        @if (l.needs_review) {
          <span class="status review">À vérifier</span>
        }
        @if (l.expiry_missing) {
          <span class="status review">Date à compléter</span>
        }
        @if (l.effective_expiry; as e) {
          <span class="status" [class]="cls()">
            @if (l.status !== 'OK') {
              {{ statusLabels[l.status] }} ·
            }
            {{ expiryShort[l.expiry_kind] }} {{ e.date | frDate: 'short' }}
          </span>
        }
      </div>
    </a>
  `,
})
export class LotRow {
  readonly lot = input.required<Lot>();
  readonly showLocation = input(true);

  protected readonly statusLabels = STATUS_LABELS;
  protected readonly expiryShort = EXPIRY_KIND_SHORT;
  protected readonly cls = computed(() => `status ${statusClass(this.lot())}`);
  protected readonly quantityText = computed(() => {
    const l = this.lot();
    return l.tracking_mode === 'PRESENCE'
      ? PRESENCE_LABELS[l.presence ?? 'AVAILABLE']
      : formatQuantity(l.quantity, l.measure);
  });
}
