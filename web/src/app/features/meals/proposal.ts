import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { ProposedMeal } from '../../api/models/proposed-meal';
import { AiService } from '../../api/services/ai.service';
import { DataVersion } from '../../core/data-version';
import { formatDayHeader, formatShortDate, weekStart } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';

/**
 * Proposition de menus par l'IA (R-PLA-64) : repas, raisons, éléments écartés (R-PLA-67), recettes nouvelles en
 * brouillon (R-PLA-66) ; on l'accepte en tout ou repas par repas.
 */
@Component({
  selector: 'app-proposal',
  imports: [RouterLink, MatButtonModule, MatCheckboxModule, MatIconModule, MatProgressBarModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (proposal.value(); as p) {
        <div class="page-header">
          <a
            mat-icon-button
            routerLink="/repas"
            [queryParams]="{ semaine: week(p.start) }"
            aria-label="Retour au calendrier"
            ><mat-icon>arrow_back</mat-icon></a
          >
          <h1>Menus proposés du {{ shortDate(p.start) }} au {{ shortDate(p.end) }}</h1>
        </div>
        @if (p.prompt) {
          <p class="muted">Consigne : « {{ p.prompt }} »</p>
        }
        @if (p.new_recipes.length) {
          <section class="card notice">
            <h2 class="widget-title">Recettes nouvelles à relire</h2>
            <p>Les repas qui les utilisent s'acceptent une fois la recette validée.</p>
            @for (r of p.new_recipes; track r.index) {
              <div class="new">
                <span>{{ r.name }}</span>
                @if (r.recipe_id) {
                  <span class="status past">Validée</span>
                } @else if (r.draft_id) {
                  <a mat-stroked-button [routerLink]="['/recettes/import', r.draft_id]">Relire</a>
                }
              </div>
            }
          </section>
        }
        <section class="card">
          @for (m of p.meals; track m.date + m.slot_id) {
            <div class="meal" [class.accepted]="m.accepted">
              <div class="meal-head">
                @if (!m.accepted && m.ready && open(p.status)) {
                  <mat-checkbox
                    [checked]="isSelected(m)"
                    (change)="select(m, $event.checked)"
                    [attr.aria-label]="'Choisir : ' + label(m)"
                  />
                }
                <span class="name">{{ label(m) }}</span>
                @if (m.accepted) {
                  <span class="status past">Accepté</span>
                } @else if (!m.ready) {
                  <span class="status soon">Recette à relire</span>
                }
              </div>
              <div class="items">
                @for (i of m.items; track $index) {
                  <span>{{ i.name }}</span>
                }
              </div>
              @if (m.reason) {
                <div class="meta">{{ m.reason }}</div>
              }
            </div>
          } @empty {
            <p>Aucun repas retenu.</p>
          }
        </section>
        @if (p.rejected.length) {
          <section class="card">
            <h2 class="widget-title">Écarté par le contrôle</h2>
            @for (r of p.rejected; track $index) {
              <div class="meta">
                {{ r.date ? shortDate(r.date) + ' ' : ''
                }}{{ r.slot_name ? r.slot_name.toLowerCase() + ' : ' : '' }}{{ r.reason }}
              </div>
            }
          </section>
        }
        @if (open(p.status)) {
          <div class="actions bottom">
            <button mat-flat-button (click)="accept(true)" [disabled]="busy() || !selected().size">
              Accepter la sélection ({{ selected().size }})
            </button>
            <button mat-stroked-button (click)="accept(false)" [disabled]="busy() || !readyCount()">
              Accepter tout ce qui est prêt
            </button>
            <span class="spacer"></span>
            <button mat-button class="destructive" (click)="discard()">
              Écarter la proposition
            </button>
          </div>
        }
      } @else if (proposal.error()) {
        <p class="error">{{ message(proposal.error()) }}</p>
      } @else {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .notice {
      background: var(--mat-sys-tertiary-container);
      color: var(--mat-sys-on-tertiary-container);
    }
    .new {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
      padding: 0.25rem 0;
    }
    .meal {
      padding: 0.75rem 0;
      border-bottom: 1px solid var(--mat-sys-outline-variant);
    }
    .meal:last-child {
      border-bottom: none;
    }
    .meal.accepted {
      opacity: 0.7;
    }
    .meal-head {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .name {
      font: var(--mat-sys-title-small);
    }
    .items {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem 1rem;
      margin: 0.25rem 0;
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .bottom {
      margin-bottom: 2rem;
      align-items: center;
    }
    .spacer {
      flex: 1;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class ProposalPage {
  private readonly ai = inject(AiService);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  private readonly version = inject(DataVersion);

  readonly id = input.required<string>();

  protected readonly shortDate = formatShortDate;
  protected readonly week = weekStart;
  protected readonly message = errorMessage;
  protected readonly busy = signal(false);
  protected readonly selected = signal(new Set<string>());

  protected readonly proposal = rxResource({
    params: () => ({ id: this.id(), v: this.version.value() }),
    stream: ({ params }) => this.ai.getAiPlan({ id: params.id }),
  });

  protected readonly readyCount = computed(
    () => (this.proposal.value()?.meals ?? []).filter((m) => m.ready && !m.accepted).length,
  );

  protected open(status: string): boolean {
    return status === 'PENDING' || status === 'PARTIALLY_ACCEPTED';
  }

  private key(m: ProposedMeal): string {
    return `${m.date}|${m.slot_id}`;
  }

  protected label(m: ProposedMeal): string {
    return `${formatDayHeader(m.date)} · ${m.slot_name.toLowerCase()}`;
  }

  protected isSelected(m: ProposedMeal): boolean {
    return this.selected().has(this.key(m));
  }

  protected select(m: ProposedMeal, checked: boolean): void {
    this.selected.update((set) => {
      const next = new Set(set);
      if (checked) next.add(this.key(m));
      else next.delete(this.key(m));
      return next;
    });
  }

  protected async accept(onlySelected: boolean): Promise<void> {
    const meals = onlySelected
      ? [...this.selected()].map((k) => {
          const [date, slot_id] = k.split('|');
          return { date, slot_id };
        })
      : [];
    this.busy.set(true);
    try {
      const result = await firstValueFrom(this.ai.acceptAiPlan({ id: this.id(), body: { meals } }));
      const skipped = result.skipped.length ? ` · ${result.skipped.join(' ; ')}` : '';
      this.snack.open(`${result.accepted} repas ajouté(s) au calendrier${skipped}`, 'Fermer', {
        duration: 6000,
      });
      this.selected.set(new Set());
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }

  protected async discard(): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Écarter la proposition',
      message:
        'Les repas non acceptés ne seront pas ajoutés. Les recettes en brouillon restent à relire ou abandonner.',
      action: 'Écarter',
      destructive: true,
    });
    if (!ok) return;
    await firstValueFrom(this.ai.discardAiPlan({ id: this.id() }));
    this.version.bump();
    await this.router.navigate(['/repas']);
  }
}
