import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatTabsModule } from '@angular/material/tabs';
import { Router, RouterLink } from '@angular/router';
import { debounceTime, firstValueFrom } from 'rxjs';

import { FoodGroup } from '../../api/models/food-group';
import { FoodsService } from '../../api/services/foods.service';
import { DataVersion } from '../../core/data-version';
import { MEASURE_LABELS, TRACKING_LABELS } from '../../core/labels';
import { GroupsStore } from '../../shared/stores';
import { GroupDialog } from './group-dialog';
import { NewFood, NewFoodDialog } from '../../shared/new-food-dialog';

/** Aliments (ALI-01, ALI-06) : recherche dans le référentiel, et arborescence des familles. */
@Component({
  selector: 'app-foods',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    MatSlideToggleModule,
    MatTabsModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Aliments</h1>
        <button mat-stroked-button (click)="editGroup()">
          <mat-icon>create_new_folder</mat-icon>Famille
        </button>
        <button mat-flat-button (click)="newFood()"><mat-icon>add</mat-icon>Aliment</button>
      </div>
      <mat-tab-group>
        <mat-tab label="Aliments">
          <div class="filters">
            <mat-form-field class="search">
              <mat-icon matPrefix>search</mat-icon>
              <input
                matInput
                placeholder="Nom ou alias"
                [ngModel]="q()"
                (ngModelChange)="q.set($event)"
              />
            </mat-form-field>
            <mat-form-field>
              <mat-label>Famille</mat-label>
              <mat-select [ngModel]="group()" (ngModelChange)="group.set($event)">
                <mat-option [value]="null">Toutes</mat-option>
                @for (g of groups.list(); track g.id) {
                  <mat-option [value]="g.id">{{ g.path }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-slide-toggle [ngModel]="hidden()" (ngModelChange)="hidden.set($event)"
              >Aliments masqués</mat-slide-toggle
            >
          </div>
          @if (foods.isLoading()) {
            <mat-progress-bar mode="indeterminate" />
          }
          <div class="list">
            @for (f of foods.value() ?? []; track f.id) {
              <a class="lot-row" [routerLink]="['/aliments', f.id]">
                <div>
                  <div class="name">{{ f.name }}</div>
                  <div class="meta">{{ f.group_path }}</div>
                </div>
                <div class="right meta">
                  <span>{{ measureLabels[f.effective.stock_measure] }}</span>
                  @if (f.effective.tracking_mode === 'PRESENCE') {
                    <span>{{ trackingLabels.PRESENCE }}</span>
                  }
                  @if (f.hidden) {
                    <span class="status">masqué</span>
                  }
                </div>
              </a>
            } @empty {
              @if (foods.hasValue()) {
                <div class="empty">
                  Aucun aliment. Taper au moins un mot, ou choisir une famille.
                </div>
              }
            }
          </div>
        </mat-tab>
        <mat-tab label="Familles">
          <div class="list groups">
            @for (g of groups.list(); track g.id) {
              <button
                class="lot-row group"
                [style.padding-left.rem]="0.75 + depth(g) * 1.25"
                (click)="editGroup(g)"
              >
                <div>
                  <div class="name">{{ g.name }}</div>
                  <div class="meta">
                    {{ g.food_count }} aliment{{ g.food_count > 1 ? 's' : '' }}
                    @if (g.ciqual_code) {
                      · Ciqual {{ g.ciqual_code }}
                    }
                  </div>
                </div>
                <div class="right meta">
                  <span>{{ measureLabels[g.effective.stock_measure] }}</span>
                  @if (g.effective.tracking_mode === 'PRESENCE') {
                    <span>{{ trackingLabels.PRESENCE }}</span>
                  }
                </div>
              </button>
            }
          </div>
        </mat-tab>
      </mat-tab-group>
    </div>
  `,
  styles: `
    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem 1rem;
      margin: 1rem 0;
    }
    .search {
      width: 16rem;
    }
    .list {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }
    .groups {
      margin-top: 1rem;
    }
    .group {
      border: none;
      text-align: left;
      cursor: pointer;
      font: inherit;
    }
  `,
})
export class FoodsPage {
  private readonly api = inject(FoodsService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);
  protected readonly groups = inject(GroupsStore);

  protected readonly measureLabels = MEASURE_LABELS;
  protected readonly trackingLabels = TRACKING_LABELS;

  protected readonly q = signal('');
  protected readonly group = signal<string | null>(null);
  protected readonly hidden = signal(false);
  private readonly query = toSignal(toObservable(this.q).pipe(debounceTime(200)), {
    initialValue: '',
  });

  protected readonly foods = rxResource({
    params: () => ({
      q: this.query().trim(),
      group: this.group(),
      hidden: this.hidden(),
      v: this.version.value(),
    }),
    stream: ({ params }) =>
      this.api.searchFoods({
        q: params.q || undefined,
        group_id: params.group ?? undefined,
        include_hidden: params.hidden,
      }),
  });

  private readonly depths = computed(
    () => new Map(this.groups.list().map((g) => [g.id, g.path.split(' › ').length - 1])),
  );

  protected depth(group: FoodGroup): number {
    return this.depths().get(group.id) ?? 0;
  }

  protected async editGroup(group?: FoodGroup): Promise<void> {
    const ref = this.dialog.open(GroupDialog, {
      data: { group },
      width: '40rem',
      maxWidth: '100vw',
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async newFood(): Promise<void> {
    const ref = this.dialog.open(NewFoodDialog, {
      data: { groupId: this.group() },
      width: '30rem',
    });
    const created: NewFood | undefined = await firstValueFrom(ref.afterClosed());
    if (created) {
      this.version.bump();
      await this.router.navigate(['/aliments', created.id]);
    }
  }
}
