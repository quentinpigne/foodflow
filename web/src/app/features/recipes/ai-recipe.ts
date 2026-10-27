import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { RecipeIdeaMode } from '../../api/models/recipe-idea-mode';
import { AiService } from '../../api/services/ai.service';
import { RecipesService } from '../../api/services/recipes.service';
import { errorMessage } from '../../core/http-errors';
import { confirmAiSend } from '../../shared/ai-preview-dialog';

const MODES: { value: RecipeIdeaMode; label: string; hint: string }[] = [
  {
    value: 'PROMPT',
    label: 'Une idée',
    hint: 'Décrivez la recette voulue : « un plat végétarien rapide pour 4 ».',
  },
  {
    value: 'STOCK',
    label: 'Le stock à écouler',
    hint: 'Une recette avec ce qui est à consommer en priorité et les restes.',
  },
  {
    value: 'VARIANT',
    label: 'Une variante',
    hint: 'Partir d’une de vos recettes : « plus légère », « sans gluten »…',
  },
  {
    value: 'SURPRISE',
    label: 'Au choix de l’IA',
    hint: 'L’IA choisit d’après le stock et les goûts du foyer.',
  },
];

/** Créer une recette avec l'IA (REC-19) : elle arrive en brouillon, relu comme un import (R-REC-82). */
@Component({
  selector: 'app-ai-recipe',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatButtonToggleModule,
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
        <a mat-icon-button routerLink="/recettes" aria-label="Retour aux recettes"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Créer une recette avec l'IA</h1>
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
            <mat-button-toggle-group
              name="mode"
              [ngModel]="mode()"
              (ngModelChange)="mode.set($event)"
              aria-label="Point de départ"
            >
              @for (m of modes; track m.value) {
                <mat-button-toggle [value]="m.value">{{ m.label }}</mat-button-toggle>
              }
            </mat-button-toggle-group>
            <p class="muted hint">{{ hint() }}</p>
            @if (mode() === 'VARIANT') {
              <mat-form-field class="full-width">
                <mat-label>Recette de départ</mat-label>
                <mat-select
                  name="recipe"
                  [ngModel]="recipeId()"
                  (ngModelChange)="recipeId.set($event)"
                >
                  @for (r of recipes.value() ?? []; track r.id) {
                    <mat-option [value]="r.id">{{ r.name }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            }
            <mat-form-field class="full-width">
              <mat-label>{{
                mode() === 'PROMPT' || mode() === 'VARIANT' ? 'Consigne' : 'Consigne (facultative)'
              }}</mat-label>
              <textarea
                matInput
                rows="3"
                name="prompt"
                [ngModel]="prompt()"
                (ngModelChange)="prompt.set($event)"
              ></textarea>
            </mat-form-field>
            @if (busy()) {
              <mat-progress-bar mode="indeterminate" />
              <p class="muted">L'IA rédige la recette… cela peut prendre une minute.</p>
            }
            @if (error()) {
              <p class="error">{{ error() }}</p>
            }
            <div class="actions">
              <button mat-flat-button type="submit" [disabled]="busy()">
                <mat-icon>auto_awesome</mat-icon>Proposer une recette
              </button>
            </div>
            <p class="muted small">
              {{ s.used_this_month }} appel(s) sur {{ s.monthly_limit }} ce mois-ci.
            </p>
          </form>
        }
      } @else {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styles: `
    .hint {
      margin: 0.75rem 0 1rem;
    }
    .small {
      font: var(--mat-sys-body-small);
      margin-top: 0.75rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class AiRecipePage {
  private readonly ai = inject(AiService);
  private readonly recipesApi = inject(RecipesService);
  private readonly dialog = inject(MatDialog);
  private readonly router = inject(Router);

  protected readonly modes = MODES;
  protected readonly mode = signal<RecipeIdeaMode>('PROMPT');
  protected readonly prompt = signal('');
  protected readonly recipeId = signal<string | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  protected readonly settings = rxResource({ stream: () => this.ai.getAiSettings() });
  protected readonly recipes = rxResource({ stream: () => this.recipesApi.searchRecipes({}) });

  protected hint(): string {
    return MODES.find((m) => m.value === this.mode())?.hint ?? '';
  }

  protected async send(): Promise<void> {
    const body = { mode: this.mode(), prompt: this.prompt() || null, recipe_id: this.recipeId() };
    this.error.set(null);
    try {
      const preview = await firstValueFrom(this.ai.previewAiRecipe({ body }));
      if (!(await confirmAiSend(this.dialog, preview))) return;
      this.busy.set(true);
      const draft = await firstValueFrom(this.ai.createAiRecipe({ body }));
      await this.router.navigate(['/recettes/import', draft.id]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
