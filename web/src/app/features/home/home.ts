import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';

import { DashboardService } from '../../api/services/dashboard.service';
import { DataVersion } from '../../core/data-version';
import { LotRow } from '../../shared/lot-row';
import { AddLot } from '../stock/add-lot';

/** Accueil (TDB-01) : ce qui demande une action dans le stock. */
@Component({
  selector: 'app-home',
  imports: [RouterLink, MatButtonModule, MatIconModule, MatProgressBarModule, LotRow],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Accueil</h1>
      </div>
      @if (board.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (board.hasValue()) {
        @let b = board.value();
        <div class="tiles">
          <a class="card tile" routerLink="/stock">
            <span class="value">{{ b.lot_count }}</span>
            <span class="muted">lots en stock</span>
          </a>
          <div class="card tile">
            <span class="value soon">{{ b.soon_count }}</span>
            <span class="muted">à consommer bientôt</span>
          </div>
          <div class="card tile">
            <span class="value expired">{{ b.expired_count }}</span>
            <span class="muted">à date dépassée</span>
          </div>
        </div>

        <section class="card">
          <h2 class="widget-title">À consommer en priorité</h2>
          @if (b.priority.length) {
            <div class="lot-list">
              @for (lot of b.priority; track lot.id) {
                <app-lot-row [lot]="lot" />
              }
            </div>
          } @else {
            <div class="empty">
              Rien ne presse.
              @if (!b.lot_count) {
                <div>
                  <button mat-button (click)="addLot.open()">
                    <mat-icon>add</mat-icon> Ajouter au stock
                  </button>
                </div>
              }
            </div>
          }
        </section>
      }
    </div>
  `,
  styles: `
    .tiles {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 0.75rem;
      margin-bottom: 1rem;
    }
    .tile {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      color: inherit;
      text-decoration: none;
    }
    .value {
      font: var(--mat-sys-headline-medium);
    }
    .value.soon {
      color: var(--mat-sys-tertiary);
    }
    .value.expired {
      color: var(--mat-sys-error);
    }
  `,
})
export class HomePage {
  private readonly api = inject(DashboardService);
  private readonly version = inject(DataVersion);
  protected readonly addLot = inject(AddLot);

  protected readonly board = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.getDashboard(),
  });
}
