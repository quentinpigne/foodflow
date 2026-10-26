import { NgTemplateOutlet } from '@angular/common';
import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { Meal } from '../../api/models/meal';
import { MealSlotId } from '../../api/models/meal-slot-id';
import { PlanningService } from '../../api/services/planning.service';
import { DataVersion } from '../../core/data-version';
import {
  addDays,
  formatDayHeader,
  formatShortDate,
  formatShortDay,
  todayIso,
  weekStart,
} from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { MEAL_STATUS_LABELS, signalText } from '../../core/labels';
import { Layout } from '../../core/layout';
import { formatPortions } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';

/** Calendrier des repas (PLA-03) : la semaine, jour par jour et créneau par créneau, avec les signalements. */
@Component({
  selector: 'app-plan',
  imports: [
    NgTemplateOutlet,
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatProgressBarModule,
    MatTooltipModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page wide">
      <div class="page-header">
        <h1>Repas</h1>
        <div class="week-nav">
          <button mat-icon-button aria-label="Semaine précédente" (click)="move(-7)">
            <mat-icon>chevron_left</mat-icon>
          </button>
          <span class="week-label">Semaine du {{ shortDate(start()) }}</span>
          <button mat-icon-button aria-label="Semaine suivante" (click)="move(7)">
            <mat-icon>chevron_right</mat-icon>
          </button>
          @if (start() !== thisWeek) {
            <button mat-button (click)="go(thisWeek)">Cette semaine</button>
          }
        </div>
      </div>
      <div class="actions toolbar">
        <button mat-stroked-button (click)="copyPrevious()" [disabled]="busy()">
          <mat-icon>content_copy</mat-icon>Reprendre la semaine précédente
        </button>
        @if (toValidate().length) {
          <button mat-flat-button (click)="validatePast()" [disabled]="busy()">
            <mat-icon>task_alt</mat-icon
            >{{
              toValidate().length === 1
                ? 'Valider le repas passé'
                : 'Valider les ' + toValidate().length + ' repas passés'
            }}
          </button>
        }
        <a mat-button routerLink="/foyer"><mat-icon>tune</mat-icon>Foyer et semaine type</a>
      </div>
      @if (plan.isLoading() || busy()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (plan.value(); as p) {
        @if (layout.mobile()) {
          @for (day of p.days; track day.date) {
            <h2 class="day" [class.today]="day.date === today">{{ dayHeader(day.date) }}</h2>
            <div class="lot-list">
              @for (s of p.slots; track s.id) {
                @if (mealAt(day.date, s.id); as meal) {
                  <ng-container
                    *ngTemplateOutlet="card; context: { $implicit: meal, slot: s.name }"
                  />
                } @else {
                  <button class="add" type="button" (click)="create(day.date, s.id)">
                    <mat-icon>add</mat-icon>{{ s.name }}
                  </button>
                }
              }
            </div>
          }
        } @else {
          <div
            class="grid"
            [style.grid-template-columns]="'8rem repeat(' + p.days.length + ', 1fr)'"
          >
            <div></div>
            @for (day of p.days; track day.date) {
              <div class="head" [class.today]="day.date === today">{{ shortDay(day.date) }}</div>
            }
            @for (s of p.slots; track s.id) {
              <div class="slot">{{ s.name }}</div>
              @for (day of p.days; track day.date) {
                <div class="cell">
                  @if (mealAt(day.date, s.id); as meal) {
                    <ng-container *ngTemplateOutlet="card; context: { $implicit: meal }" />
                  } @else {
                    <button
                      class="add"
                      type="button"
                      [attr.aria-label]="'Ajouter : ' + s.name + ', ' + dayHeader(day.date)"
                      (click)="create(day.date, s.id)"
                    >
                      <mat-icon>add</mat-icon>
                    </button>
                  }
                </div>
              }
            }
          </div>
        }
      }
    </div>

    <ng-template #card let-meal let-slot="slot">
      <a
        class="meal"
        [routerLink]="['/repas', meal.id]"
        [class.done]="meal.status === 'DONE'"
        [class.off]="meal.status === 'SKIPPED' || meal.status === 'OUT_OF_HOME'"
      >
        <div class="meal-head">
          @if (slot) {
            <span class="slot-name">{{ slot }}</span>
          }
          @if (meal.status !== 'PLANNED') {
            <span class="status" [class.past]="meal.status === 'DONE'">{{
              statusLabel(meal)
            }}</span>
          }
          @if (meal.signals.length) {
            <span
              class="status review"
              [matTooltip]="signalsText(meal)"
              [attr.aria-label]="signalsText(meal)"
              ><mat-icon>warning</mat-icon>{{ meal.signals.length }}</span
            >
          }
        </div>
        @for (item of meal.items; track item.id) {
          <div class="item">{{ item.name }}</div>
        } @empty {
          @if (meal.status === 'PLANNED') {
            <div class="item muted">À compléter</div>
          }
        }
        <div class="meta">{{ portions(meal.adult_equivalents) }} éq. adulte</div>
      </a>
    </ng-template>
  `,
  styles: `
    .wide {
      max-width: 90rem;
    }
    .week-nav {
      display: flex;
      align-items: center;
      gap: 0.25rem;
    }
    .week-label {
      font: var(--mat-sys-title-small);
    }
    .toolbar {
      margin-bottom: 1rem;
    }
    .grid {
      display: grid;
      gap: 0.5rem;
      align-items: stretch;
    }
    .head {
      font: var(--mat-sys-title-small);
      text-align: center;
      padding: 0.25rem;
      border-radius: var(--mat-sys-corner-small);
    }
    .head.today,
    .day.today {
      color: var(--mat-sys-primary);
    }
    .slot {
      font: var(--mat-sys-label-large);
      color: var(--mat-sys-on-surface-variant);
      padding-top: 0.75rem;
    }
    .cell {
      display: flex;
      min-height: 5.5rem;
    }
    .day {
      font: var(--mat-sys-title-medium);
      margin: 1.25rem 0 0.5rem;
    }
    .add {
      flex: 1;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 0.25rem;
      min-height: 3rem;
      border: 1px dashed var(--mat-sys-outline-variant);
      border-radius: var(--mat-sys-corner-medium);
      background: none;
      color: var(--mat-sys-on-surface-variant);
      font: var(--mat-sys-label-large);
      cursor: pointer;
    }
    .add:hover {
      background: var(--mat-sys-surface-container-low);
    }
    .meal {
      flex: 1;
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      padding: 0.5rem 0.625rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-surface-container-low);
      color: inherit;
      text-decoration: none;
      min-width: 0;
    }
    .meal:hover {
      background: var(--mat-sys-surface-container);
    }
    .meal.done {
      background: var(--mat-sys-secondary-container);
    }
    .meal.off {
      opacity: 0.7;
    }
    .meal-head {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      align-items: center;
    }
    .slot-name {
      font: var(--mat-sys-label-large);
      margin-right: auto;
    }
    .status mat-icon {
      font-size: 1rem;
      width: 1rem;
      height: 1rem;
    }
    .item {
      font: var(--mat-sys-body-medium);
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
      margin-top: auto;
    }
  `,
})
export class PlanPage {
  private readonly api = inject(PlanningService);
  private readonly router = inject(Router);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly version = inject(DataVersion);
  protected readonly layout = inject(Layout);

  /** Semaine affichée (`?semaine=AAAA-MM-JJ`), la semaine en cours par défaut. */
  readonly semaine = input<string>();

  protected readonly today = todayIso();
  protected readonly thisWeek = weekStart(this.today);
  protected readonly start = computed(() => weekStart(this.semaine() ?? this.today));
  protected readonly busy = signal(false);
  protected readonly statusLabels = MEAL_STATUS_LABELS;
  protected readonly portions = formatPortions;
  protected readonly shortDay = formatShortDay;
  protected readonly shortDate = formatShortDate;
  protected readonly dayHeader = formatDayHeader;

  protected readonly plan = rxResource({
    params: () => ({ start: this.start(), v: this.version.value() }),
    stream: ({ params }) => this.api.getPlan({ start: params.start, days: 7 }),
  });

  private readonly members = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listMembers(),
  });

  /** Repas passés encore prévus : à valider (R-PLA-35). */
  protected readonly toValidate = computed(() =>
    (this.plan.value()?.days ?? [])
      .flatMap((d) => d.meals)
      .filter((m) => m.status === 'PLANNED' && m.date < this.today),
  );

  protected statusLabel(meal: Meal): string {
    return this.statusLabels[meal.status];
  }

  protected mealAt(date: string, slot: MealSlotId): Meal | undefined {
    return this.plan
      .value()
      ?.days.find((d) => d.date === date)
      ?.meals.find((m) => m.slot_id === slot);
  }

  protected signalsText(meal: Meal): string {
    const name = (id: string) =>
      this.members.value()?.find((m) => m.id === id)?.name ?? 'Un convive';
    return meal.signals.map((s) => signalText(s, formatPortions, name)).join(' · ');
  }

  protected move(days: number): void {
    this.go(addDays(this.start(), days));
  }

  protected go(start: string): void {
    void this.router.navigate(['/repas'], {
      queryParams: { semaine: start === this.thisWeek ? null : start },
    });
  }

  protected async create(date: string, slot: MealSlotId): Promise<void> {
    this.busy.set(true);
    try {
      const created = await firstValueFrom(this.api.createMeal({ body: { date, slot_id: slot } }));
      await this.router.navigate(['/repas', created.id]);
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }

  protected async copyPrevious(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Reprendre la semaine précédente',
      message:
        'Ses repas sont recopiés comme repas prévus dans les créneaux vides de cette semaine, avec les convives par défaut. Les restes ne sont pas recopiés.',
      action: 'Reprendre',
    });
    if (!ok) return;
    await this.run(async () => {
      const result = await firstValueFrom(
        this.api.copyPlan({ body: { from: addDays(this.start(), -7), to: this.start() } }),
      );
      const skipped = result.skipped ? `, ${result.skipped} créneau(x) déjà rempli(s)` : '';
      return `${result.created} repas repris${skipped}`;
    });
  }

  protected async validatePast(): Promise<void> {
    const meals = this.toValidate();
    const ok = await confirm(this.dialog, {
      title: 'Valider les repas passés',
      message: `${meals.length} repas seront validés avec les sorties de stock proposées, sans reste déclaré. Un repas se valide aussi un par un pour ajuster.`,
      action: 'Valider',
    });
    if (!ok) return;
    await this.run(async () => {
      const result = await firstValueFrom(
        this.api.validateMeals({ body: { meal_ids: meals.map((m) => m.id) } }),
      );
      const failed = result.failed.length ? ` · ${result.failed.length} en échec` : '';
      return `${result.validated.length} repas validés${failed}`;
    });
  }

  private async run(action: () => Promise<string>): Promise<void> {
    this.busy.set(true);
    try {
      this.snack.open(await action(), undefined, { duration: 4000 });
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }
}
