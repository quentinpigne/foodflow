import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';
import { Observable, firstValueFrom } from 'rxjs';

import { quantityText } from './ingredients';
import { RecipeDetail } from '../../api/models/recipe-detail';
import { ValuesBasis } from '../../api/models/values-basis';
import { RecipesService } from '../../api/services/recipes.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { NUTRITION_GROUP_LABELS, RECIPE_KIND_LABELS, formatMinutes } from '../../core/labels';
import { NUTRIENTS, formatNutrient } from '../../core/nutrients';
import { formatIn, formatQuantity, parseQuantity, UNITS } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { choosePhoto } from '../../shared/photo-dialog';

const BASIS_LABELS: Record<ValuesBasis, string> = {
  PORTION: 'par portion adulte',
  PER100_G: 'pour 100 g',
  PER100_ML: 'pour 100 mL',
  PIECE: 'par pièce',
};

/** Fiche d'une recette (REC-02, REC-03, REC-05) : mise à l'échelle, valeurs par portion, portions des groupes. */
@Component({
  selector: 'app-recipe-page',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    MatTooltipModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/recettes" aria-label="Retour aux recettes"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>{{ recipe.value()?.name ?? 'Recette' }}</h1>
        @if (recipe.hasValue()) {
          @let r = recipe.value();
          <a
            mat-flat-button
            [routerLink]="['/recettes', r.id, 'cuisine']"
            [queryParams]="{ portions: servingsText() }"
          >
            <mat-icon>skillet</mat-icon>Cuisiner
          </a>
          <a mat-stroked-button [routerLink]="['/recettes', r.id, 'modifier']"
            ><mat-icon>edit</mat-icon>Modifier</a
          >
          <button mat-icon-button [matMenuTriggerFor]="more" aria-label="Autres actions">
            <mat-icon>more_vert</mat-icon>
          </button>
          <mat-menu #more>
            <button mat-menu-item (click)="duplicate(r)">
              <mat-icon>content_copy</mat-icon>Faire une variante
            </button>
            <button mat-menu-item (click)="archive(r)">
              <mat-icon>{{ r.archived ? 'unarchive' : 'archive' }}</mat-icon
              >{{ r.archived ? 'Désarchiver' : 'Archiver' }}
            </button>
            <a mat-menu-item [href]="'/api/v1/recipes/' + r.id + '/json-ld'" download
              ><mat-icon>download</mat-icon>Exporter (JSON-LD)</a
            >
            <button mat-menu-item (click)="changePhoto(r)">
              <mat-icon>image</mat-icon>Changer de photo
            </button>
            @if (r.has_image) {
              <button mat-menu-item (click)="removeImage(r)">
                <mat-icon>hide_image</mat-icon>Retirer la photo
              </button>
            }
            <button mat-menu-item (click)="remove(r)"><mat-icon>delete</mat-icon>Supprimer</button>
          </mat-menu>
        }
      </div>
      @if (recipe.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (recipe.hasValue()) {
        @let r = recipe.value();
        <div class="layout">
          <div class="side">
            @if (r.has_image) {
              <img
                class="photo"
                [src]="'/api/v1/recipes/' + r.id + '/image?v=' + imageVersion()"
                alt=""
              />
            }
            <section class="card">
              <div class="facts">
                <span>{{ kindLabels[r.kind] }}</span>
                <span
                  ><mat-icon inline>schedule</mat-icon>
                  {{ formatMinutes(r.times.total_active_minutes) }} actif</span
                >
                <span
                  ><mat-icon inline>timer</mat-icon> {{ formatMinutes(r.times.total_minutes) }} au
                  total</span
                >
                @if (r.fridge_days) {
                  <span><mat-icon inline>kitchen</mat-icon> {{ r.fridge_days }} j au frigo</span>
                }
                @if (r.freezable) {
                  <span><mat-icon inline>ac_unit</mat-icon> congelable {{ r.frozen_days }} j</span>
                }
                @if (r.finishing_minutes) {
                  <span>finition {{ formatMinutes(r.finishing_minutes) }}</span>
                }
              </div>
              @if (r.yield.kind === 'PORTIONS') {
                <mat-form-field class="servings">
                  <mat-label>Pour (équivalents adulte)</mat-label>
                  <input
                    matInput
                    inputmode="decimal"
                    [ngModel]="servingsText()"
                    (ngModelChange)="servingsText.set($event)"
                  />
                  <mat-hint
                    >La recette est pour {{ formatPortions(r.yield.portions) }} portions.</mat-hint
                  >
                </mat-form-field>
              } @else {
                <p class="muted">
                  Rendement : {{ formatQuantity(r.yield.amount, r.yield.measure) }}
                </p>
              }
              <h2 class="widget-title">Ingrédients</h2>
              <ul class="ingredients">
                @for (i of r.ingredients; track i.id) {
                  @if (i.section && isFirstOfSection(r, $index)) {
                    <li class="section">{{ i.section }}</li>
                  }
                  <li [class.optional]="i.optional">
                    <span class="qty">{{ quantityText(i) }}</span>
                    @if (i.target.kind === 'RECIPE') {
                      <a [routerLink]="['/recettes', i.target.id]">{{ i.target_name }}</a>
                    } @else if (i.target.kind === 'FOOD') {
                      <a [routerLink]="['/aliments', i.target.id]">{{ i.target_name }}</a>
                    } @else {
                      <span>{{ i.target_name }} <span class="muted">(au choix)</span></span>
                    }
                    @if (i.note) {
                      <span class="muted">, {{ i.note }}</span>
                    }
                    @if (i.optional) {
                      <span class="muted"> (facultatif)</span>
                    }
                    @if (i.unconvertible) {
                      <mat-icon
                        inline
                        class="warn"
                        matTooltip="Quantité non convertible : les valeurs nutritionnelles sont incomplètes"
                        >warning</mat-icon
                      >
                    }
                  </li>
                }
              </ul>
            </section>
          </div>
          <div class="main">
            @if (r.description) {
              <p>{{ r.description }}</p>
            }
            @if (r.notes) {
              <p class="notes"><mat-icon inline>sticky_note_2</mat-icon> {{ r.notes }}</p>
            }
            <section class="card">
              <h2 class="widget-title">Préparation</h2>
              <ol class="steps">
                @for (s of r.steps; track $index) {
                  @if (s.section && s.section !== r.steps[$index - 1]?.section) {
                    <li class="section">{{ s.section }}</li>
                  }
                  <li>
                    {{ s.text }}
                    @if (s.timer_minutes) {
                      <span class="muted">({{ formatMinutes(s.timer_minutes) }})</span>
                    }
                  </li>
                } @empty {
                  <li class="muted">Pas d'étape.</li>
                }
              </ol>
            </section>
            <section class="card">
              <h2 class="widget-title">
                Valeurs nutritionnelles <span class="muted">{{ basisLabels[r.values_basis] }}</span>
              </h2>
              <table class="values">
                @for (n of nutrientRows(); track n.code) {
                  <tr>
                    <th>{{ n.label }}</th>
                    <td class="amount">
                      @if (n.value; as v) {
                        {{ v.estimated ? '≈ ' : '' }}{{ formatNutrient(v.value) }} {{ n.unit }}
                      } @else {
                        —
                      }
                    </td>
                    <td>
                      @if (n.value?.missing?.length) {
                        <span
                          class="status review"
                          [matTooltip]="'Manque : ' + n.value!.missing.join(', ')"
                          >incomplet ({{ n.value!.missing.length }})</span
                        >
                      }
                    </td>
                  </tr>
                }
              </table>
              @if (r.group_portions.length) {
                <h3 class="subtitle">Portions des groupes</h3>
                <div class="groups">
                  @for (g of r.group_portions; track g.group) {
                    <span class="status"
                      >{{ groupLabels[g.group] }} : {{ formatPortions(g.portions) }}</span
                    >
                  }
                </div>
              }
            </section>
            @if (r.used_by.length || r.source_url || r.leftover_food_name) {
              <section class="card links">
                @if (r.used_by.length) {
                  <div>
                    Utilisée par :
                    @for (u of r.used_by; track u.id) {
                      <a [routerLink]="['/recettes', u.id]">{{ u.name }}</a
                      >{{ $last ? '' : ', ' }}
                    }
                  </div>
                }
                @if (r.leftover_food_name) {
                  <div>Restes : {{ r.leftover_food_name }}</div>
                }
                @if (r.source_url) {
                  <div>
                    Source :
                    <a [href]="r.source_url" target="_blank" rel="noopener">{{ r.source_url }}</a>
                  </div>
                }
              </section>
            }
          </div>
        </div>
      }
    </div>
  `,
  styles: `
    .layout {
      display: grid;
      grid-template-columns: minmax(18rem, 24rem) 1fr;
      gap: 1rem;
      align-items: start;
    }
    @media (max-width: 899.98px) {
      .layout {
        grid-template-columns: 1fr;
      }
    }
    .side,
    .main {
      display: flex;
      flex-direction: column;
      gap: 1rem;
    }
    .photo {
      width: 100%;
      border-radius: var(--mat-sys-corner-large);
      object-fit: cover;
      max-height: 16rem;
    }
    .facts {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem 1rem;
      margin-bottom: 1rem;
      color: var(--mat-sys-on-surface-variant);
    }
    .servings {
      width: 100%;
      margin-bottom: 1rem;
    }
    .ingredients,
    .steps {
      margin: 0;
      padding-left: 1.25rem;
      display: flex;
      flex-direction: column;
      gap: 0.375rem;
    }
    .ingredients {
      list-style: none;
      padding-left: 0;
    }
    .section {
      list-style: none;
      font: var(--mat-sys-title-small);
      margin-top: 0.5rem;
    }
    .steps .section {
      margin-left: -1.25rem;
    }
    .qty {
      font-variant-numeric: tabular-nums;
      font-weight: 500;
      margin-right: 0.25rem;
    }
    .optional {
      opacity: 0.75;
    }
    .warn {
      color: var(--mat-sys-error);
    }
    .notes {
      background: var(--mat-sys-tertiary-container);
      color: var(--mat-sys-on-tertiary-container);
      padding: 0.75rem;
      border-radius: var(--mat-sys-corner-medium);
    }
    .values {
      width: 100%;
      border-collapse: collapse;
    }
    .values th {
      text-align: left;
      font-weight: normal;
      padding: 0.25rem 0;
    }
    .values td {
      padding: 0.25rem 0.5rem;
    }
    .subtitle {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.5rem;
    }
    .groups {
      display: flex;
      flex-wrap: wrap;
      gap: 0.5rem;
    }
    .links {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
    }
  `,
})
export class RecipePage {
  private readonly api = inject(RecipesService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);

  readonly id = input.required<string>();

  protected readonly kindLabels = RECIPE_KIND_LABELS;
  protected readonly groupLabels = NUTRITION_GROUP_LABELS;
  protected readonly basisLabels = BASIS_LABELS;
  protected readonly formatMinutes = formatMinutes;
  protected readonly formatNutrient = formatNutrient;
  protected readonly formatQuantity = formatQuantity;

  protected readonly servingsText = signal('');
  protected readonly imageVersion = signal(0);
  private readonly servings = computed(() => parseQuantity(this.servingsText(), 'COUNT'));

  protected readonly recipe = rxResource({
    params: () => ({ id: this.id(), servings: this.servings(), v: this.version.value() }),
    stream: ({ params }) =>
      this.api.getRecipe({ id: params.id, servings: params.servings || undefined }),
  });

  protected readonly nutrientRows = computed(() => {
    const values = this.recipe.value()?.nutrients ?? [];
    return NUTRIENTS.map((n) => ({ ...n, value: values.find((v) => v.code === n.code) ?? null }));
  });

  constructor() {
    // Par défaut, la recette telle qu'écrite : son nombre de portions.
    effect(() => {
      const r = this.recipe.value();
      if (r && !this.servingsText() && r.yield.kind === 'PORTIONS') {
        this.servingsText.set(this.formatPortions(r.yield.portions));
      }
    });
  }

  protected formatPortions(raw: number): string {
    return formatIn(raw, UNITS.COUNT[0]);
  }

  protected isFirstOfSection(r: RecipeDetail, index: number): boolean {
    return index === 0 || r.ingredients[index - 1].section !== r.ingredients[index].section;
  }

  protected readonly quantityText = quantityText;

  protected async duplicate(r: RecipeDetail): Promise<void> {
    const created = await this.run(this.api.duplicateRecipe({ id: r.id }), 'Variante créée');
    if (created) {
      await this.router.navigate(['/recettes', (created as { id: string }).id, 'modifier']);
    }
  }

  protected async archive(r: RecipeDetail): Promise<void> {
    await this.run(
      this.api.archiveRecipe({ id: r.id, body: { archived: !r.archived } }),
      r.archived ? 'Recette désarchivée' : 'Recette archivée',
    );
  }

  protected async remove(r: RecipeDetail): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer ${r.name} ?`,
      message:
        'Une recette utilisée comme sous-recette ne peut pas être supprimée : l’archiver plutôt.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok && (await this.run(this.api.deleteRecipe({ id: r.id }), 'Recette supprimée')) !== null) {
      await this.router.navigate(['/recettes']);
    }
  }

  /** Photo proposée, adresse saisie ou fichier envoyé (R-REC-73). */
  protected async changePhoto(r: RecipeDetail): Promise<void> {
    const chosen = await choosePhoto(this.dialog, r.name);
    if (!chosen) return;
    const request =
      'file' in chosen
        ? this.api.uploadRecipeImage({ id: r.id, body: { image: chosen.file } as never })
        : this.api.chooseRecipePhoto({ id: r.id, body: { url: chosen.url } });
    if ((await this.run(request, 'Photo enregistrée')) !== null) {
      this.imageVersion.update((v) => v + 1);
    }
  }

  protected async removeImage(r: RecipeDetail): Promise<void> {
    await this.run(this.api.deleteRecipeImage({ id: r.id }), 'Photo retirée');
  }

  private async run(request: Observable<unknown>, message: string): Promise<unknown> {
    try {
      const result = await firstValueFrom(request);
      this.version.bump();
      this.snack.open(message, undefined, { duration: 3000 });
      return result ?? true;
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
      return null;
    }
  }
}
