import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
  untracked,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { AiService } from '../../api/services/ai.service';
import { PlanningService } from '../../api/services/planning.service';
import { addDays, formatShortDate, todayIso, weekStart } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { confirmAiSend } from '../../shared/ai-preview-dialog';

/**
 * Proposer des menus avec l'IA (PLA-12) : les créneaux vides de la période, une consigne ; le résultat est une
 * proposition à accepter repas par repas (R-PLA-64).
 */
@Component({
  selector: 'app-ai-plan',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a
          mat-icon-button
          routerLink="/repas"
          [queryParams]="{ semaine: start() }"
          aria-label="Retour au calendrier"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Proposer des menus avec l'IA</h1>
      </div>
      @if (settings.value(); as s) {
        @if (!s.ready) {
          <section class="card">
            <p>
              L'IA n'est pas activée. Choisissez le fournisseur, le modèle et la clé d'accès dans
              les réglages.
            </p>
            <a mat-stroked-button routerLink="/reglages">Réglages</a>
          </section>
        } @else {
          <form class="card" (ngSubmit)="send()">
            <p class="muted">
              L'IA remplit les créneaux vides (ou prévus sans rien dedans) avec vos recettes, vos
              restes et au plus {{ s.new_recipes_per_plan }} recette(s) nouvelle(s), à relire avant
              d'accepter les repas qui les utilisent. Rien n'est écrit dans le calendrier avant
              votre accord.
            </p>
            <div class="row">
              <mat-form-field>
                <mat-label>À partir du</mat-label>
                <input
                  matInput
                  type="date"
                  name="start"
                  [ngModel]="start()"
                  (ngModelChange)="startDate.set($event)"
                />
              </mat-form-field>
              <mat-form-field>
                <mat-label>Durée</mat-label>
                <mat-select name="days" [ngModel]="days()" (ngModelChange)="days.set($event)">
                  <mat-option [value]="7">Une semaine</mat-option>
                  <mat-option [value]="14">Deux semaines</mat-option>
                  <mat-option [value]="3">Trois jours</mat-option>
                </mat-select>
              </mat-form-field>
            </div>
            <p class="label">Créneaux à remplir</p>
            <div class="row">
              @for (slot of template.value() ?? []; track slot.slot_id) {
                <mat-checkbox
                  [checked]="chosen().has(slot.slot_id)"
                  (change)="toggle(slot.slot_id, $event.checked)"
                  >{{ slot.name }}</mat-checkbox
                >
              }
            </div>
            <mat-form-field class="full-width">
              <mat-label>Consigne (facultative)</mat-label>
              <textarea
                matInput
                rows="2"
                name="prompt"
                placeholder="Semaine légère, poisson deux fois, cuisine italienne…"
                [ngModel]="prompt()"
                (ngModelChange)="prompt.set($event)"
              ></textarea>
            </mat-form-field>
            @if (busy()) {
              <mat-progress-bar mode="indeterminate" />
              <p class="muted">L'IA compose les menus… cela peut prendre une à deux minutes.</p>
            }
            @if (error()) {
              <p class="error">{{ error() }}</p>
            }
            <div class="actions">
              <button mat-flat-button type="submit" [disabled]="busy() || !chosen().size">
                <mat-icon>auto_awesome</mat-icon>Proposer les menus du {{ shortDate(start()) }}
              </button>
            </div>
          </form>
        }
      }
    </div>
  `,
  styles: `
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem var(--field-gap);
      margin-bottom: 0.75rem;
    }
    .label {
      font: var(--mat-sys-title-small);
      margin: 0.5rem 0;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AiPlanPage {
  private readonly ai = inject(AiService);
  private readonly planning = inject(PlanningService);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);

  /** `?semaine=AAAA-MM-JJ` : la semaine affichée au calendrier ; la semaine prochaine par défaut. */
  readonly semaine = input<string>();

  protected readonly startDate = signal<string | null>(null);
  protected readonly start = computed(
    () => this.startDate() ?? this.semaine() ?? addDays(weekStart(todayIso()), 7),
  );
  protected readonly days = signal(7);
  protected readonly prompt = signal('');
  protected readonly chosen = signal(new Set<string>());
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly shortDate = formatShortDate;

  protected readonly settings = rxResource({ stream: () => this.ai.getAiSettings() });
  protected readonly template = rxResource({ stream: () => this.planning.getWeekTemplate() });

  constructor() {
    // Par défaut : les créneaux qui ont une composition type (déjeuner, dîner), pas les repas habituels.
    effect(() => {
      const template = this.template.value();
      if (template) {
        untracked(() =>
          this.chosen.set(new Set(template.filter((t) => t.roles.length).map((t) => t.slot_id))),
        );
      }
    });
  }

  protected toggle(slot: string, checked: boolean): void {
    this.chosen.update((set) => {
      const next = new Set(set);
      if (checked) next.add(slot);
      else next.delete(slot);
      return next;
    });
  }

  protected async send(): Promise<void> {
    const body = {
      start: this.start(),
      days: this.days(),
      slot_ids: [...this.chosen()],
      prompt: this.prompt() || null,
    };
    this.error.set(null);
    try {
      const preview = await firstValueFrom(this.ai.previewAiPlan({ body }));
      if (!(await confirmAiSend(this.dialog, preview))) return;
      this.busy.set(true);
      const proposal = await firstValueFrom(this.ai.createAiPlan({ body }));
      await this.router.navigate(['/repas/propositions', proposal.id]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
