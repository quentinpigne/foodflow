import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { RouterLink } from '@angular/router';
import { debounceTime } from 'rxjs';

import { RecipeKind } from '../../api/models/recipe-kind';
import { RecipesService } from '../../api/services/recipes.service';
import { DataVersion } from '../../core/data-version';
import { RECIPE_KINDS, RECIPE_KIND_LABELS, formatMinutes } from '../../core/labels';
import { formatNutrient } from '../../core/nutrients';

/** Recettes (REC-09) : recherche par nom, étiquette ou ingrédient, filtres, « avec ce que j'ai ». */
@Component({
  selector: 'app-recipes',
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
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Recettes</h1>
        <a mat-stroked-button routerLink="/recettes/ia"
          ><mat-icon>auto_awesome</mat-icon>Avec l'IA</a
        >
        <a mat-stroked-button routerLink="/recettes/import"><mat-icon>link</mat-icon>Importer</a>
        <a mat-flat-button routerLink="/recettes/nouvelle"><mat-icon>add</mat-icon>Recette</a>
      </div>
      <div class="filters">
        <mat-form-field class="search">
          <mat-icon matPrefix>search</mat-icon>
          <input
            matInput
            placeholder="Nom, étiquette, ingrédient"
            [ngModel]="q()"
            (ngModelChange)="q.set($event)"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Catégorie</mat-label>
          <mat-select [ngModel]="kind()" (ngModelChange)="kind.set($event)">
            <mat-option [value]="null">Toutes</mat-option>
            @for (k of kinds; track k) {
              <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field>
          <mat-label>Étiquette</mat-label>
          <mat-select [ngModel]="tag()" (ngModelChange)="tag.set($event)">
            <mat-option [value]="null">Toutes</mat-option>
            @for (t of tags.value() ?? []; track t) {
              <mat-option [value]="t">{{ t }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field class="short">
          <mat-label>Temps actif max.</mat-label>
          <input
            matInput
            type="number"
            min="0"
            [ngModel]="maxActive()"
            (ngModelChange)="maxActive.set($event || null)"
          />
          <span matTextSuffix>min</span>
        </mat-form-field>
        <mat-slide-toggle [ngModel]="makeAhead()" (ngModelChange)="makeAhead.set($event)"
          >À l'avance</mat-slide-toggle
        >
        <mat-slide-toggle [ngModel]="inStock()" (ngModelChange)="inStock.set($event)"
          >Avec ce que j'ai</mat-slide-toggle
        >
        <mat-slide-toggle [ngModel]="archived()" (ngModelChange)="archived.set($event)"
          >Archivées</mat-slide-toggle
        >
      </div>
      @if (recipes.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      <div class="grid">
        @for (r of recipes.value() ?? []; track r.id) {
          <a class="card recipe" [routerLink]="['/recettes', r.id]">
            @if (r.has_image) {
              <img [src]="'/api/v1/recipes/' + r.id + '/image'" alt="" loading="lazy" />
            } @else {
              <div class="placeholder">
                <span class="material-symbols-outlined">restaurant</span>
              </div>
            }
            <div class="body">
              <div class="name">{{ r.name }}</div>
              <div class="meta">
                {{ kindLabels[r.kind] }} · {{ formatMinutes(r.times.total_active_minutes) }} actif ·
                {{ formatMinutes(r.times.total_minutes) }} au total
              </div>
              <div class="meta">
                @if (r.energy !== null && r.energy !== undefined) {
                  {{ formatNutrient(r.energy) }} kcal{{
                    r.yield.kind === 'PORTIONS' ? ' / portion' : ' / 100'
                  }}
                  ·
                }
                @if (r.make_ahead) {
                  à l'avance ·
                }
                @if (r.archived) {
                  archivée ·
                }
                @for (t of r.tags; track t) {
                  #{{ t }}
                }
              </div>
              @if (r.coverage; as c) {
                <div class="status" [class.soon]="c.covered < c.total">
                  {{ c.covered }} / {{ c.total }} ingrédients en stock
                </div>
              }
            </div>
          </a>
        } @empty {
          @if (recipes.hasValue()) {
            <div class="empty">Aucune recette.</div>
          }
        }
      </div>
    </div>
  `,
  styles: `
    .filters {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem 1rem;
      margin-bottom: 1rem;
    }
    .search {
      width: 16rem;
    }
    .short {
      width: 9rem;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
      gap: 1rem;
    }
    .recipe {
      display: flex;
      flex-direction: column;
      padding: 0;
      overflow: hidden;
      color: inherit;
      text-decoration: none;
    }
    .recipe:hover {
      background: var(--mat-sys-surface-container);
    }
    img,
    .placeholder {
      width: 100%;
      height: 9rem;
      object-fit: cover;
      background: var(--mat-sys-surface-container-high);
    }
    .placeholder {
      display: flex;
      align-items: center;
      justify-content: center;
      color: var(--mat-sys-on-surface-variant);
    }
    .placeholder span {
      font-family: 'Material Symbols Outlined';
      font-size: 2.5rem;
    }
    .body {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      padding: 0.75rem 1rem 1rem;
    }
    .name {
      font: var(--mat-sys-title-medium);
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .status {
      align-self: flex-start;
    }
  `,
})
export class RecipesPage {
  private readonly api = inject(RecipesService);
  private readonly version = inject(DataVersion);

  protected readonly kinds = RECIPE_KINDS;
  protected readonly kindLabels = RECIPE_KIND_LABELS;
  protected readonly formatMinutes = formatMinutes;
  protected readonly formatNutrient = formatNutrient;

  protected readonly q = signal('');
  protected readonly kind = signal<RecipeKind | null>(null);
  protected readonly tag = signal<string | null>(null);
  protected readonly maxActive = signal<number | null>(null);
  protected readonly makeAhead = signal(false);
  protected readonly inStock = signal(false);
  protected readonly archived = signal(false);
  private readonly query = toSignal(toObservable(this.q).pipe(debounceTime(250)), {
    initialValue: '',
  });

  protected readonly tags = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listRecipeTags(),
  });

  protected readonly recipes = rxResource({
    params: () => ({
      q: this.query().trim() || undefined,
      kind: this.kind() ?? undefined,
      tag: this.tag() ?? undefined,
      max_active: this.maxActive() ?? undefined,
      make_ahead: this.makeAhead(),
      in_stock: this.inStock(),
      include_archived: this.archived(),
      version: this.version.value(),
    }),
    stream: ({ params }) => {
      const { version, ...query } = params;
      void version;
      return this.api.searchRecipes(query);
    },
  });
}
