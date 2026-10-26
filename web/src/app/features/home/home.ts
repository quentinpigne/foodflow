import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';

import { DashboardService } from '../../api/services/dashboard.service';
import { weekStart } from '../../core/dates';
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

        @if (b.meals_to_validate) {
          <a
            class="card validate"
            routerLink="/repas"
            [queryParams]="{ semaine: week(b.oldest_to_validate ?? '') }"
          >
            <mat-icon>task_alt</mat-icon>
            <span>
              {{
                b.meals_to_validate === 1
                  ? '1 repas passé à valider'
                  : b.meals_to_validate + ' repas passés à valider'
              }}
            </span>
          </a>
        }

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
    .validate {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      margin-bottom: 1rem;
      color: var(--mat-sys-on-tertiary-container);
      background: var(--mat-sys-tertiary-container);
      text-decoration: none;
      font: var(--mat-sys-title-small);
    }
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
  protected readonly week = weekStart;
  private readonly version = inject(DataVersion);
  protected readonly addLot = inject(AddLot);

  protected readonly board = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.getDashboard(),
  });
}
