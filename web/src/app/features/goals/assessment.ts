import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router, RouterLink } from '@angular/router';

import { CriterionAssessment } from '../../api/models/criterion-assessment';
import { GoalsService } from '../../api/services/goals.service';
import { formatCriterionValue, describeCriterion } from '../../core/criteria';
import { DataVersion } from '../../core/data-version';
import { addDays, formatDayHeader, formatShortDate, todayIso, weekStart } from '../../core/dates';
import { ASSESSMENT_LABELS, IMPORTANCE_LABELS } from '../../core/labels';
import { formatNutrient } from '../../core/nutrients';
import { GroupsStore } from '../../shared/stores';

/**
 * Bilan nutritionnel d'une semaine (NUT-04) : prévu ou réel, critère par critère, par équivalent adulte. Pas de
 * score global (R-NUT-44).
 */
@Component({
  selector: 'app-assessment',
  imports: [
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
    MatIconModule,
    MatProgressBarModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Bilan</h1>
        <div class="week-nav">
          <button mat-icon-button aria-label="Semaine précédente" (click)="go(-7)">
            <mat-icon>chevron_left</mat-icon>
          </button>
          <span>Semaine du {{ shortDate(start()) }}</span>
          <button mat-icon-button aria-label="Semaine suivante" (click)="go(7)">
            <mat-icon>chevron_right</mat-icon>
          </button>
        </div>
      </div>
      <mat-button-toggle-group
        [value]="actual() ? 'actual' : 'planned'"
        (change)="setActual($event.value === 'actual')"
        aria-label="Bilan prévu ou réel"
      >
        <mat-button-toggle value="planned">Prévu</mat-button-toggle>
        <mat-button-toggle value="actual">Réel</mat-button-toggle>
      </mat-button-toggle-group>
      <p class="muted intro">
        {{
          actual()
            ? 'D’après les repas validés : ce qui a été réellement utilisé et mangé.'
            : 'D’après les repas prévus, sans les restes qu’on cuisine pour plus tard.'
        }}
        Valeurs par équivalent adulte. Le sel et les matières grasses des ingrédients « à volonté »
        ne sont pas comptés (R-NUT-23).
      </p>
      @if (report.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (report.value(); as r) {
        @if (emptySlots(); as empty) {
          <section class="card notice">
            {{ empty }} créneau(x) sans repas cette semaine : le bilan est incomplet (détail jour
            par jour ci-dessous).
          </section>
        }
        @if (!r.goal_id) {
          <section class="card">
            <p>Aucun objectif nutritionnel n'est en vigueur cette semaine.</p>
            <a mat-stroked-button routerLink="/objectifs">Définir un objectif</a>
          </section>
        } @else {
          <section class="card">
            @for (c of r.criteria; track $index) {
              <div class="criterion">
                <div class="what">
                  <div class="name">{{ describe(c) }}</div>
                  <div class="meta">
                    {{ importanceLabels[c.criterion.importance] }} · cible {{ target(c) }}
                  </div>
                  @if (c.missing.length) {
                    <div class="meta missing">
                      Manque : {{ c.missing.slice(0, 4).join(', ')
                      }}{{ c.missing.length > 4 ? '…' : '' }}
                    </div>
                  }
                </div>
                <div class="value">
                  <strong>{{ value(c) }}</strong>
                  <span class="status" [class]="statusClass(c)">{{ statusLabels[c.status] }}</span>
                </div>
              </div>
            }
          </section>
        }
        <section class="card">
          <h2 class="widget-title">Jour par jour</h2>
          @for (day of r.days; track day.date) {
            <div class="day">
              <span>{{ dayHeader(day.date) }}</span>
              <span class="muted">{{ day.meals }} repas</span>
              <span>{{
                day.energy !== null && day.energy !== undefined
                  ? nutrient(day.energy) + ' kcal'
                  : '—'
              }}</span>
            </div>
            @if (day.missing.length) {
              <div class="meta missing">{{ day.missing.join(' · ') }}</div>
            }
          }
        </section>
      }
    </div>
  `,
  styles: `
    .week-nav {
      display: flex;
      align-items: center;
      gap: 0.25rem;
    }
    .intro {
      margin: 0.75rem 0 1rem;
    }
    .card {
      margin-bottom: 1rem;
    }
    .criterion {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 1rem;
      padding: 0.75rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }
    .criterion:last-child {
      border-bottom: none;
    }
    .name {
      font: var(--mat-sys-title-small);
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .missing {
      color: var(--mat-sys-tertiary);
    }
    .notice {
      background: var(--mat-sys-tertiary-container);
      color: var(--mat-sys-on-tertiary-container);
    }
    .value {
      display: flex;
      flex-direction: column;
      align-items: flex-end;
      gap: 0.25rem;
      white-space: nowrap;
    }
    .status.met {
      background: var(--mat-sys-primary-container);
      color: var(--mat-sys-on-primary-container);
    }
    .day {
      display: grid;
      grid-template-columns: 1fr auto 7rem;
      gap: 1rem;
      padding: 0.5rem 0 0.25rem;
    }
    .day span:last-child {
      text-align: right;
    }
  `,
})
export class AssessmentPage {
  private readonly api = inject(GoalsService);
  private readonly router = inject(Router);
  private readonly version = inject(DataVersion);
  private readonly groups = inject(GroupsStore);

  /** `?semaine=AAAA-MM-JJ` ; `?reel=1` pour le bilan réel. */
  readonly semaine = input<string>();
  readonly reel = input<string>();

  protected readonly start = computed(() => weekStart(this.semaine() ?? todayIso()));
  protected readonly actual = computed(() => this.reel() === '1');
  protected readonly statusLabels = ASSESSMENT_LABELS;
  protected readonly importanceLabels = IMPORTANCE_LABELS;
  protected readonly shortDate = formatShortDate;
  protected readonly dayHeader = formatDayHeader;
  protected readonly nutrient = formatNutrient;

  protected readonly report = rxResource({
    params: () => ({ start: this.start(), actual: this.actual(), v: this.version.value() }),
    stream: ({ params }) =>
      this.api.getAssessment({ start: params.start, days: 7, actual: params.actual }),
  });

  /** Créneaux sans repas de la semaine, comptés une fois (R-NUT-32). */
  protected readonly emptySlots = computed(
    () =>
      (this.report.value()?.days ?? []).flatMap((d) =>
        d.missing.filter((m) => m.endsWith(': aucun repas')),
      ).length,
  );

  /** Ce qui manque à un critère, hors créneaux vides (déjà signalés une fois) : des valeurs inconnues. */
  protected ingredientsMissing(c: CriterionAssessment): string {
    const missing = c.missing.filter((m) => !m.endsWith(': aucun repas'));
    return missing.length ? missing.slice(0, 5).join(', ') + (missing.length > 5 ? '…' : '') : '';
  }

  protected describe(c: CriterionAssessment): string {
    return describeCriterion(
      c.criterion,
      (id) => this.groups.list().find((g) => g.id === id)?.name ?? '',
    );
  }

  protected value(c: CriterionAssessment): string {
    return formatCriterionValue(c.criterion, c.value);
  }

  protected target(c: CriterionAssessment): string {
    const min =
      c.min !== null && c.min !== undefined ? formatCriterionValue(c.criterion, c.min) : null;
    const max =
      c.max !== null && c.max !== undefined ? formatCriterionValue(c.criterion, c.max) : null;
    if (min && max) return min === max ? min : `${min} à ${max}`;
    return min ? `au moins ${min}` : max ? `au plus ${max}` : '—';
  }

  protected statusClass(c: CriterionAssessment): string {
    return {
      MET: 'status met',
      BELOW: 'status soon',
      ABOVE: 'status soon',
      INCOMPLETE: 'status',
      NOT_APPLICABLE: 'status',
    }[c.status];
  }

  protected go(days: number): void {
    this.navigate(addDays(this.start(), days), this.actual());
  }

  protected setActual(actual: boolean): void {
    this.navigate(this.start(), actual);
  }

  private navigate(start: string, actual: boolean): void {
    void this.router.navigate(['/bilan'], {
      queryParams: {
        semaine: start === weekStart(todayIso()) ? null : start,
        reel: actual ? 1 : null,
      },
    });
  }
}
