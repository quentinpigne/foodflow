import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  linkedSignal,
  signal,
  untracked,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom, of } from 'rxjs';

import { Confidence } from '../../api/models/confidence';
import { IngredientInput } from '../../api/models/ingredient-input';
import { Measure } from '../../api/models/measure';
import { RecipeInput } from '../../api/models/recipe-input';
import { RecipeKind } from '../../api/models/recipe-kind';
import { Target } from '../../api/models/target';
import { Unit } from '../../api/models/unit';
import { FoodsService } from '../../api/services/foods.service';
import { RecipesService } from '../../api/services/recipes.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import {
  CONFIDENCE_LABELS,
  MEASURE_LABELS,
  RECIPE_KINDS,
  RECIPE_KIND_LABELS,
} from '../../core/labels';
import { UNITS, defaultUnit, formatIn, parseQuantity, quantityInput } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { ItemPicker, StockItem } from '../../shared/item-picker';
import { TargetPicker } from '../../shared/target-picker';
import { choosePhoto } from '../../shared/photo-dialog';

interface IngredientDraft {
  key: number;
  section: string;
  target: Target | null;
  targetName: string;
  quantity: string;
  unitId: string | null;
  unitLabel: string | null;
  toTaste: boolean;
  optional: boolean;
  fixed: boolean;
  note: string;
  sourceLine: string | null;
  importName: string | null;
  confidence: Confidence | null;
}

interface StepDraft {
  key: number;
  section: string;
  text: string;
  timer: number | null;
}

let keys = 0;

/**
 * Éditeur de recette (REC-01, REC-05, REC-06) : nouvelle recette, modification, ou revue d'un import
 * (REC-07) avant sa validation (R-REC-61 à R-REC-66).
 */
@Component({
  selector: 'app-recipe-editor',
  imports: [
    FormsModule,
    RouterLink,
    ItemPicker,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    MatTooltipModule,
    TargetPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button [routerLink]="back()" aria-label="Retour"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>
          {{ draftId() ? 'Revue de l’import' : id() ? 'Modifier la recette' : 'Nouvelle recette' }}
        </h1>
      </div>
      @if (loading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (existing().length) {
        <p class="card warning">
          Cette adresse a déjà été importée :
          @for (e of existing(); track e.id) {
            <a [routerLink]="['/recettes', e.id]">{{ e.name }}</a
            >{{ $last ? '.' : ', ' }}
          }
          On peut tout de même créer une autre recette.
        </p>
      }
      @if (draftId(); as draft) {
        <section class="card photo-card">
          @if (draftHasImage()) {
            <img
              class="draft-photo"
              [src]="'/api/v1/recipe-drafts/' + draft + '/image?v=' + draftImageVersion()"
              alt=""
            />
          } @else {
            <p class="muted">Pas de photo.</p>
          }
          <button mat-stroked-button type="button" (click)="changeDraftPhoto(draft)">
            <mat-icon>image</mat-icon>Changer de photo
          </button>
        </section>
      }
      <form (ngSubmit)="save()">
        <section class="card">
          <div class="form-grid">
            <mat-form-field class="wide">
              <mat-label>Nom</mat-label>
              <input
                matInput
                name="name"
                [ngModel]="name()"
                (ngModelChange)="name.set($event)"
                required
                maxlength="200"
              />
            </mat-form-field>
            <mat-form-field>
              <mat-label>Catégorie</mat-label>
              <mat-select name="kind" [ngModel]="kind()" (ngModelChange)="kind.set($event)">
                @for (k of kinds; track k) {
                  <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Étiquettes</mat-label>
              <input matInput name="tags" [ngModel]="tags()" (ngModelChange)="tags.set($event)" />
              <mat-hint>Séparées par des virgules</mat-hint>
            </mat-form-field>
          </div>
          <div class="row">
            <mat-button-toggle-group
              name="yieldKind"
              [ngModel]="yieldKind()"
              (ngModelChange)="yieldKind.set($event)"
            >
              <mat-button-toggle value="PORTIONS">En portions</mat-button-toggle>
              <mat-button-toggle value="AMOUNT">En quantité</mat-button-toggle>
            </mat-button-toggle-group>
            @if (yieldKind() === 'PORTIONS') {
              <mat-form-field>
                <mat-label>Portions adulte</mat-label>
                <input
                  matInput
                  name="portions"
                  inputmode="decimal"
                  [ngModel]="portions()"
                  (ngModelChange)="portions.set($event)"
                />
              </mat-form-field>
            } @else {
              <mat-form-field>
                <mat-label>Mesure</mat-label>
                <mat-select
                  name="yieldMeasure"
                  [ngModel]="yieldMeasure()"
                  (ngModelChange)="yieldMeasure.set($event)"
                >
                  @for (m of measures; track m) {
                    <mat-option [value]="m">{{ measureLabels[m] }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Quantité produite</mat-label>
                <input
                  matInput
                  name="yieldAmount"
                  [ngModel]="yieldAmount()"
                  (ngModelChange)="yieldAmount.set($event)"
                  placeholder="900 g"
                />
              </mat-form-field>
            }
          </div>
          <div class="form-grid">
            <mat-form-field>
              <mat-label>Temps actif</mat-label>
              <input
                matInput
                name="active"
                type="number"
                min="0"
                [ngModel]="active()"
                (ngModelChange)="active.set($event)"
              />
              <span matTextSuffix>min</span>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Temps passif (cuisson, repos)</mat-label>
              <input
                matInput
                name="passive"
                type="number"
                min="0"
                [ngModel]="passive()"
                (ngModelChange)="passive.set($event)"
              />
              <span matTextSuffix>min</span>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Conservation au frigo</mat-label>
              <input
                matInput
                name="fridge"
                type="number"
                min="1"
                [ngModel]="fridgeDays()"
                (ngModelChange)="fridgeDays.set($event || null)"
                placeholder="ne se prépare pas à l'avance"
              />
              <span matTextSuffix>j</span>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Temps de finition</mat-label>
              <input
                matInput
                name="finishing"
                type="number"
                min="0"
                [ngModel]="finishing()"
                (ngModelChange)="finishing.set($event ?? null)"
              />
              <span matTextSuffix>min</span>
            </mat-form-field>
            <div class="inline">
              <mat-checkbox
                name="freezable"
                [ngModel]="freezable()"
                (ngModelChange)="freezable.set($event)"
                >Congelable</mat-checkbox
              >
              @if (freezable()) {
                <mat-form-field class="short">
                  <mat-label>Au congélateur</mat-label>
                  <input
                    matInput
                    name="frozen"
                    type="number"
                    min="1"
                    [ngModel]="frozenDays()"
                    (ngModelChange)="frozenDays.set($event || null)"
                  />
                  <span matTextSuffix>j</span>
                </mat-form-field>
              }
            </div>
          </div>
          <div class="leftover">
            @if (leftoverName()) {
              <div class="inline">
                <span>Restes : {{ leftoverName() }}</span>
                <button
                  mat-icon-button
                  type="button"
                  aria-label="Retirer l'aliment de reste"
                  (click)="setLeftover(null)"
                >
                  <mat-icon>close</mat-icon>
                </button>
              </div>
            } @else {
              <app-item-picker
                label="Aliment de reste (facultatif)"
                [foodsOnly]="true"
                (picked)="setLeftover($event)"
              />
            }
          </div>
        </section>

        <section class="card">
          <h2 class="widget-title">Ingrédients</h2>
          @if (draftId()) {
            <p class="muted">
              Les lignes les moins sûres sont en tête. Chaque ingrédient doit viser un aliment, une
              famille ou une recette.
            </p>
          }
          <div class="ingredients">
            @for (i of ingredients(); track i.key; let index = $index) {
              <div class="ingredient">
                @if (i.sourceLine) {
                  <div class="source muted">
                    « {{ i.sourceLine }} »
                    @if (i.confidence) {
                      <span
                        class="status"
                        [class.review]="i.confidence === 'NONE' || i.confidence === 'LOW'"
                        [class.soon]="i.confidence === 'MEDIUM'"
                      >
                        {{ confidenceLabels[i.confidence] }}
                      </span>
                    }
                  </div>
                }
                <div class="line">
                  <mat-form-field class="qty">
                    <mat-label>Qté</mat-label>
                    <input
                      matInput
                      [name]="'q' + i.key"
                      inputmode="decimal"
                      [ngModel]="i.quantity"
                      (ngModelChange)="patch(index, { quantity: $event })"
                      [disabled]="i.toTaste"
                    />
                  </mat-form-field>
                  <mat-form-field class="unit">
                    <mat-label>Unité</mat-label>
                    <mat-select
                      [name]="'u' + i.key"
                      [ngModel]="i.unitId"
                      (ngModelChange)="patch(index, { unitId: $event, unitLabel: null })"
                      [disabled]="i.toTaste"
                    >
                      <mat-option [value]="null">{{
                        i.unitLabel ?? (i.target?.kind === 'RECIPE' ? 'portions' : 'pièce')
                      }}</mat-option>
                      @for (u of unitsFor(i); track u.id) {
                        <mat-option [value]="u.id">{{ u.abbreviation ?? u.name }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                  <app-target-picker
                    class="target"
                    [name]="i.targetName"
                    [exclude]="id() ?? null"
                    (picked)="
                      patch(index, {
                        target: $event.target,
                        targetName: $event.name,
                        confidence: null,
                      })
                    "
                  />
                  <button
                    mat-icon-button
                    type="button"
                    aria-label="Retirer l'ingrédient"
                    (click)="removeIngredient(index)"
                  >
                    <mat-icon>delete</mat-icon>
                  </button>
                </div>
                <div class="line options">
                  <mat-checkbox
                    [name]="'t' + i.key"
                    [ngModel]="i.toTaste"
                    (ngModelChange)="
                      patch(index, { toTaste: $event, quantity: $event ? '' : i.quantity })
                    "
                    >À volonté</mat-checkbox
                  >
                  <mat-checkbox
                    [name]="'o' + i.key"
                    [ngModel]="i.optional"
                    (ngModelChange)="patch(index, { optional: $event })"
                    >Facultatif</mat-checkbox
                  >
                  <mat-checkbox
                    [name]="'f' + i.key"
                    [ngModel]="i.fixed"
                    (ngModelChange)="patch(index, { fixed: $event })"
                    matTooltip="Pas mis à l'échelle (une feuille de laurier)"
                    >Fixe</mat-checkbox
                  >
                  <mat-form-field class="note">
                    <mat-label>Précision</mat-label>
                    <input
                      matInput
                      [name]="'n' + i.key"
                      [ngModel]="i.note"
                      (ngModelChange)="patch(index, { note: $event })"
                    />
                  </mat-form-field>
                  <mat-form-field class="note">
                    <mat-label>Section</mat-label>
                    <input
                      matInput
                      [name]="'s' + i.key"
                      [ngModel]="i.section"
                      (ngModelChange)="patch(index, { section: $event })"
                      placeholder="Pour la sauce"
                    />
                  </mat-form-field>
                </div>
              </div>
            }
          </div>
          <button mat-stroked-button type="button" (click)="addIngredient()">
            <mat-icon>add</mat-icon>Ingrédient
          </button>
        </section>

        <section class="card">
          <h2 class="widget-title">Étapes</h2>
          <div class="steps">
            @for (s of steps(); track s.key; let index = $index) {
              <div class="step">
                <span class="number">{{ index + 1 }}</span>
                <mat-form-field class="grow">
                  <mat-label>Étape</mat-label>
                  <textarea
                    matInput
                    [name]="'st' + s.key"
                    rows="2"
                    [ngModel]="s.text"
                    (ngModelChange)="patchStep(index, { text: $event })"
                  ></textarea>
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Minuteur</mat-label>
                  <input
                    matInput
                    [name]="'sm' + s.key"
                    type="number"
                    min="0"
                    [ngModel]="s.timer"
                    (ngModelChange)="patchStep(index, { timer: $event || null })"
                  />
                  <span matTextSuffix>min</span>
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Section</mat-label>
                  <input
                    matInput
                    [name]="'ss' + s.key"
                    [ngModel]="s.section"
                    (ngModelChange)="patchStep(index, { section: $event })"
                  />
                </mat-form-field>
                <button
                  mat-icon-button
                  type="button"
                  aria-label="Retirer l'étape"
                  (click)="removeStep(index)"
                >
                  <mat-icon>delete</mat-icon>
                </button>
              </div>
            }
          </div>
          <button mat-stroked-button type="button" (click)="addStep()">
            <mat-icon>add</mat-icon>Étape
          </button>
        </section>

        <section class="card">
          <div class="form-grid">
            <mat-form-field class="wide">
              <mat-label>Description</mat-label>
              <textarea
                matInput
                name="description"
                rows="2"
                [ngModel]="description()"
                (ngModelChange)="description.set($event)"
              ></textarea>
            </mat-form-field>
            <mat-form-field class="wide">
              <mat-label>Notes personnelles</mat-label>
              <textarea
                matInput
                name="notes"
                rows="2"
                [ngModel]="notes()"
                (ngModelChange)="notes.set($event)"
                placeholder="Mettre moins de sel"
              ></textarea>
            </mat-form-field>
          </div>
        </section>

        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
        <div class="actions bottom">
          <button mat-flat-button type="submit" [disabled]="busy()">
            {{ draftId() ? 'Valider la recette' : 'Enregistrer' }}
          </button>
          @if (draftId()) {
            <button mat-button type="button" (click)="discard()">Abandonner l’import</button>
          }
        </div>
      </form>
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .photo-card {
      display: flex;
      align-items: center;
      gap: 1rem;
      flex-wrap: wrap;
    }
    .draft-photo {
      width: 12rem;
      max-width: 100%;
      aspect-ratio: 3 / 2;
      object-fit: cover;
      border-radius: var(--mat-sys-corner-small);
    }
    .widget-title {
      margin-bottom: 1rem;
    }
    .wide {
      grid-column: 1 / -1;
    }
    .row,
    .inline {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin: 0.5rem 0 1rem;
    }
    .short {
      width: 9rem;
    }
    .leftover {
      margin-top: var(--field-gap);
    }
    .ingredients,
    .steps {
      display: flex;
      flex-direction: column;
      gap: 1.25rem;
      margin-bottom: 1rem;
    }
    .ingredient,
    .step {
      border-bottom: 1px solid var(--mat-sys-outline-variant);
      padding-bottom: 1rem;
    }
    .source {
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-bottom: 0.75rem;
    }
    .line {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .line + .line {
      margin-top: 0.75rem;
    }
    .qty {
      width: 6rem;
    }
    .unit {
      width: 9rem;
    }
    .target {
      flex: 1;
      min-width: 14rem;
    }
    .note {
      width: 12rem;
    }
    .step {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .number {
      font: var(--mat-sys-title-medium);
      width: 1.5rem;
    }
    .grow {
      flex: 1;
      min-width: 16rem;
    }
    .warning {
      background: var(--mat-sys-tertiary-container);
      color: var(--mat-sys-on-tertiary-container);
    }
    .error {
      color: var(--mat-sys-error);
    }
    .bottom {
      margin-bottom: 2rem;
    }
  `,
})
export class RecipeEditor {
  private readonly api = inject(RecipesService);
  private readonly foods = inject(FoodsService);
  private readonly version = inject(DataVersion);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  private readonly dialog = inject(MatDialog);

  /** Recette modifiée (route `/recettes/:id/modifier`). */
  readonly id = input<string>();
  /** Brouillon d'import revu (route `/recettes/import/:draftId`). */
  readonly draftId = input<string>();

  protected readonly kinds = RECIPE_KINDS;
  protected readonly kindLabels = RECIPE_KIND_LABELS;
  protected readonly measures: Measure[] = ['MASS', 'VOLUME', 'COUNT'];
  protected readonly measureLabels = MEASURE_LABELS;
  protected readonly confidenceLabels = CONFIDENCE_LABELS;

  protected readonly name = signal('');
  protected readonly kind = signal<RecipeKind>('MAIN');
  protected readonly tags = signal('');
  protected readonly yieldKind = signal<'PORTIONS' | 'AMOUNT'>('PORTIONS');
  protected readonly portions = signal('4');
  protected readonly yieldMeasure = signal<Measure>('MASS');
  protected readonly yieldAmount = signal('');
  protected readonly active = signal(0);
  protected readonly passive = signal(0);
  protected readonly fridgeDays = signal<number | null>(null);
  protected readonly freezable = signal(false);
  protected readonly frozenDays = signal<number | null>(null);
  protected readonly finishing = signal<number | null>(null);
  protected readonly leftoverId = signal<string | null>(null);
  protected readonly leftoverName = signal<string | null>(null);
  protected readonly description = signal('');
  protected readonly notes = signal('');
  protected readonly sourceUrl = signal<string | null>(null);
  protected readonly ingredients = signal<IngredientDraft[]>([]);
  protected readonly steps = signal<StepDraft[]>([]);
  protected readonly existing = signal<{ id: string; name: string }[]>([]);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Unités propres des aliments choisis, chargées à la demande. */
  private readonly foodUnits = signal<Record<string, Unit[]>>({});

  private readonly generalUnits = rxResource({ stream: () => this.foods.listUnits() });

  private readonly recipe = rxResource({
    params: () => this.id(),
    stream: ({ params: id }) => (id ? this.api.getRecipe({ id }) : of(null)),
  });
  private readonly draft = rxResource({
    params: () => this.draftId(),
    stream: ({ params: id }) => (id ? this.api.getRecipeDraft({ id }) : of(null)),
  });

  protected readonly loading = computed(() => this.recipe.isLoading() || this.draft.isLoading());
  /** Photo du brouillon : changée sans recharger le brouillon, pour garder les corrections en cours. */
  protected readonly draftHasImage = linkedSignal(() => this.draft.value()?.has_image ?? false);
  protected readonly draftImageVersion = signal(0);
  protected readonly back = computed(() =>
    this.id() ? ['/recettes', this.id()] : this.draftId() ? ['/recettes/import'] : ['/recettes'],
  );

  constructor() {
    effect(() => {
      const r = this.recipe.value();
      if (!r) return;
      untracked(() => {
        this.name.set(r.name);
        this.kind.set(r.kind);
        this.tags.set(r.tags.join(', '));
        this.applyYield(r.yield);
        this.active.set(r.times.active_minutes);
        this.passive.set(r.times.passive_minutes);
        this.fridgeDays.set(r.fridge_days ?? null);
        this.freezable.set(r.freezable);
        this.frozenDays.set(r.frozen_days ?? null);
        this.finishing.set(r.finishing_minutes ?? null);
        this.leftoverId.set(r.leftover_food_id ?? null);
        this.leftoverName.set(r.leftover_food_name ?? null);
        this.description.set(r.description ?? '');
        this.notes.set(r.notes ?? '');
        this.sourceUrl.set(r.source_url ?? null);
        // Les quantités affichées par la fiche sont celles de la recette telle qu'écrite (sans mise à l'échelle).
        this.ingredients.set(
          r.ingredients.map((i) => ({
            key: keys++,
            section: i.section ?? '',
            target: i.target,
            targetName: i.target_name,
            quantity: i.quantity ?? '',
            unitId: i.unit_id ?? null,
            unitLabel: i.unit_id ? null : (i.unit_label ?? null),
            toTaste: i.to_taste,
            optional: i.optional,
            fixed: i.fixed,
            note: i.note ?? '',
            sourceLine: i.source_line ?? null,
            importName: null,
            confidence: null,
          })),
        );
        this.steps.set(
          r.steps.map((s) => ({
            key: keys++,
            section: s.section ?? '',
            text: s.text,
            timer: s.timer_minutes ?? null,
          })),
        );
        for (const i of r.ingredients) this.loadUnits(i.target);
      });
    });
    effect(() => {
      const d = this.draft.value();
      if (!d) return;
      untracked(() => {
        const p = d.payload;
        this.name.set(p.name);
        this.kind.set(p.kind);
        this.tags.set(p.tags.join(', '));
        this.portions.set(p.portions ?? '4');
        this.active.set(p.active_minutes);
        this.passive.set(p.passive_minutes);
        this.description.set(p.description ?? '');
        this.sourceUrl.set(p.source_url ?? null);
        this.existing.set(d.existing);
        this.ingredients.set(
          p.lines.map((l) => ({
            key: keys++,
            section: l.section ?? '',
            target: l.target ?? null,
            targetName: l.target_name ?? '',
            quantity: l.quantity ?? '',
            unitId: l.unit_id ?? null,
            unitLabel: l.unit_label ?? null,
            toTaste: l.to_taste,
            optional: false,
            fixed: false,
            note: l.note ?? '',
            sourceLine: l.source_line,
            importName: l.name,
            confidence: l.confidence,
          })),
        );
        this.steps.set(
          p.steps.map((s) => ({
            key: keys++,
            section: s.section ?? '',
            text: s.text,
            timer: s.timer_minutes ?? null,
          })),
        );
        for (const l of p.lines) if (l.target) this.loadUnits(l.target);
      });
    });
    if (!this.id() && !this.draftId()) {
      this.addIngredient();
      this.addStep();
    }
  }

  private applyYield(y: RecipeInput['yield']): void {
    this.yieldKind.set(y.kind);
    if (y.kind === 'PORTIONS') {
      this.portions.set(formatIn(y.portions, UNITS.COUNT[0]));
    } else {
      this.yieldMeasure.set(y.measure);
      this.yieldAmount.set(quantityInput(y.amount, y.measure));
    }
  }

  private loadUnits(target: Target): void {
    if (target.kind !== 'FOOD' || this.foodUnits()[target.id]) return;
    this.foodUnits.update((u) => ({ ...u, [target.id]: [] }));
    void firstValueFrom(this.foods.getFood({ id: target.id })).then((food) =>
      this.foodUnits.update((u) => ({ ...u, [target.id]: food.units })),
    );
  }

  protected unitsFor(i: IngredientDraft): Unit[] {
    const general = this.generalUnits.value() ?? [];
    const own = i.target?.kind === 'FOOD' ? (this.foodUnits()[i.target.id] ?? []) : [];
    return [...own, ...general.filter((u) => u.measure !== 'COUNT')];
  }

  protected patch(index: number, change: Partial<IngredientDraft>): void {
    if (change.target) this.loadUnits(change.target);
    this.ingredients.update((list) => list.map((i, n) => (n === index ? { ...i, ...change } : i)));
  }

  protected addIngredient(): void {
    this.ingredients.update((list) => [
      ...list,
      {
        key: keys++,
        section: list.at(-1)?.section ?? '',
        target: null,
        targetName: '',
        quantity: '',
        unitId: null,
        unitLabel: null,
        toTaste: false,
        optional: false,
        fixed: false,
        note: '',
        sourceLine: null,
        importName: null,
        confidence: null,
      },
    ]);
  }

  protected removeIngredient(index: number): void {
    this.ingredients.update((list) => list.filter((_, n) => n !== index));
  }

  protected patchStep(index: number, change: Partial<StepDraft>): void {
    this.steps.update((list) => list.map((s, n) => (n === index ? { ...s, ...change } : s)));
  }

  protected addStep(): void {
    this.steps.update((list) => [
      ...list,
      { key: keys++, section: list.at(-1)?.section ?? '', text: '', timer: null },
    ]);
  }

  protected removeStep(index: number): void {
    this.steps.update((list) => list.filter((_, n) => n !== index));
  }

  protected setLeftover(item: StockItem | null): void {
    this.leftoverId.set(item?.kind === 'food' ? item.food.id : null);
    this.leftoverName.set(item?.kind === 'food' ? item.food.name : null);
  }

  private input(): RecipeInput | string {
    const missing = this.ingredients().filter((i) => !i.target);
    if (missing.length) {
      return `Choisir l'aliment, la famille ou la recette de chaque ingrédient (${missing.length} sans cible).`;
    }
    let yieldValue: RecipeInput['yield'];
    if (this.yieldKind() === 'PORTIONS') {
      const portions = parseQuantity(this.portions(), 'COUNT');
      if (!portions) return 'Nombre de portions invalide.';
      yieldValue = { kind: 'PORTIONS', portions };
    } else {
      const amount = parseQuantity(
        this.yieldAmount(),
        this.yieldMeasure(),
        defaultUnit(this.yieldMeasure()),
      );
      if (!amount) return 'Quantité produite invalide.';
      yieldValue = { kind: 'AMOUNT', measure: this.yieldMeasure(), amount };
    }
    const ingredients: IngredientInput[] = this.ingredients().map((i) => ({
      section: i.section || null,
      target: i.target!,
      quantity: i.toTaste ? null : i.quantity.trim() || null,
      unit_id: i.toTaste ? null : i.unitId,
      unit_label: i.toTaste || i.unitId ? null : i.unitLabel,
      to_taste: i.toTaste,
      optional: i.optional,
      fixed: i.fixed,
      note: i.note || null,
      source_line: i.sourceLine,
      import_name: i.importName,
    }));
    return {
      name: this.name(),
      kind: this.kind(),
      description: this.description() || null,
      notes: this.notes() || null,
      yield: yieldValue,
      active_minutes: Number(this.active()) || 0,
      passive_minutes: Number(this.passive()) || 0,
      fridge_days: this.fridgeDays() ? Number(this.fridgeDays()) : null,
      freezable: this.freezable(),
      frozen_days: this.freezable() && this.frozenDays() ? Number(this.frozenDays()) : null,
      finishing_minutes: this.finishing() != null ? Number(this.finishing()) : null,
      leftover_food_id: this.leftoverId(),
      source_url: this.sourceUrl(),
      tags: this.tags()
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
      ingredients,
      steps: this.steps()
        .filter((s) => s.text.trim())
        .map((s) => ({
          section: s.section || null,
          text: s.text,
          timer_minutes: s.timer ? Number(s.timer) : null,
        })),
    };
  }

  protected async save(): Promise<void> {
    const body = this.input();
    if (typeof body === 'string') {
      this.error.set(body);
      return;
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      const id = this.id();
      const draft = this.draftId();
      let target: string;
      if (draft) {
        target = (await firstValueFrom(this.api.commitRecipeDraft({ id: draft, body }))).id;
      } else if (id) {
        await firstValueFrom(this.api.updateRecipe({ id, body }));
        target = id;
      } else {
        target = (await firstValueFrom(this.api.createRecipe({ body }))).id;
      }
      this.version.bump();
      if (draft && (await this.openNextDraft(draft, 'Recette enregistrée'))) {
        return;
      }
      this.snack.open('Recette enregistrée', undefined, { duration: 3000 });
      await this.router.navigate(['/recettes', target]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  protected async discard(): Promise<void> {
    const draft = this.draftId();
    if (!draft) return;
    const ok = await confirm(this.dialog, {
      title: 'Abandonner l’import ?',
      message: 'Le brouillon est supprimé ; aucune recette n’est créée.',
      action: 'Abandonner',
      destructive: true,
    });
    if (ok) {
      await firstValueFrom(this.api.discardRecipeDraft({ id: draft }));
      if (!(await this.openNextDraft(draft, 'Import abandonné'))) {
        await this.router.navigate(['/recettes/import']);
      }
    }
  }

  /** Photo proposée, adresse saisie ou fichier envoyé (R-REC-73). */
  protected async changeDraftPhoto(draft: string): Promise<void> {
    const chosen = await choosePhoto(this.dialog, this.name());
    if (!chosen) return;
    try {
      await firstValueFrom(
        'file' in chosen
          ? this.api.uploadRecipeDraftImage({ id: draft, body: { image: chosen.file } as never })
          : this.api.chooseRecipeDraftPhoto({ id: draft, body: { url: chosen.url } }),
      );
      this.draftHasImage.set(true);
      this.draftImageVersion.update((v) => v + 1);
      this.snack.open('Photo enregistrée', undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }

  /** Revue enchaînée d'un lot (R-REC-69) : ouvre le plus ancien brouillon restant, s'il y en a un. */
  private async openNextDraft(done: string, message: string): Promise<boolean> {
    const drafts = (await firstValueFrom(this.api.listRecipeDrafts())).filter((d) => d.id !== done);
    const next = drafts.at(-1);
    if (!next) {
      return false;
    }
    const left = drafts.length === 1 ? 'dernier brouillon' : `${drafts.length} brouillons restants`;
    this.snack.open(`${message} · ${left}`, undefined, { duration: 3000 });
    await this.router.navigate(['/recettes/import', next.id]);
    window.scrollTo(0, 0);
    return true;
  }
}
