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
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MatButtonToggleModule } from '@angular/material/button-toggle';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatChipsModule } from '@angular/material/chips';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatMenuModule } from '@angular/material/menu';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { debounceTime, firstValueFrom, map, of } from 'rxjs';

import { ItemSource } from '../../api/models/item-source';
import { Meal } from '../../api/models/meal';
import { MealItem } from '../../api/models/meal-item';
import { MealStatus } from '../../api/models/meal-status';
import { Measure } from '../../api/models/measure';
import { RecipeSummary } from '../../api/models/recipe-summary';
import { PlanningService } from '../../api/services/planning.service';
import { RecipesService } from '../../api/services/recipes.service';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { addDays, formatDayHeader, formatShortDate, todayIso, weekStart } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import { MEAL_STATUS_LABELS, signalText } from '../../core/labels';
import {
  formatPortions,
  formatQuantity,
  parsePortions,
  parseQuantity,
  percent,
  quantityInput,
  smallest,
  total,
} from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { ItemPicker, StockItem, itemName } from '../../shared/item-picker';

/** Un élément en cours de saisie. */
interface Row {
  key: number;
  id: string | null;
  role_id: string | null;
  source: ItemSource;
  name: string;
  recipe_id: string | null;
  /** `COOK` : portions cuisinées ; `LOT` : portions servies. */
  portions: string;
  lot_id: string | null;
  leftover_of_item_id: string | null;
  food_id: string | null;
  product_id: string | null;
  /** `FOOD` : quantité saisie. */
  amount: string;
  measure: Measure | null;
  per_adult: boolean;
  /** `COOK` : reste prévu et date limite, d'après le serveur. */
  info: string;
}

interface Guest {
  key: number;
  label: string;
  coefficient: string;
}

/** Un reste qu'on peut placer dans ce repas : prévu d'un repas antérieur, ou déjà en stock. */
interface LeftoverChoice {
  label: string;
  name: string;
  portions: number;
  leftover_of_item_id: string | null;
  lot_id: string | null;
}

let keys = 0;

/** Repas (PLA-03, R-PLA-11, R-PLA-12) : convives, horaires, éléments ; signalements (R-PLA-50). */
@Component({
  selector: 'app-meal-editor',
  imports: [
    FormsModule,
    RouterLink,
    MatAutocompleteModule,
    MatButtonModule,
    MatButtonToggleModule,
    MatCheckboxModule,
    MatChipsModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatMenuModule,
    MatProgressBarModule,
    MatSelectModule,
    ItemPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      @if (meal.value(); as m) {
        <div class="page-header">
          <a
            mat-icon-button
            [routerLink]="['/repas']"
            [queryParams]="{ semaine: week(m.date) }"
            aria-label="Retour au calendrier"
            ><mat-icon>arrow_back</mat-icon></a
          >
          <h1>{{ m.slot_name }} · {{ dayHeader(m.date) }}</h1>
          @if (m.status === 'DONE') {
            <span class="status past">Validé</span>
          }
        </div>

        @if (m.signals.length && m.status === 'PLANNED') {
          <section class="card warning">
            @for (s of m.signals; track $index) {
              <div class="signal"><mat-icon>warning</mat-icon>{{ text(s) }}</div>
            }
          </section>
        }

        @if (m.status === 'DONE') {
          <section class="card">
            <p>
              Ce repas est validé : ses sorties de stock et ses restes sont enregistrés. Pour le
              modifier, annulez d'abord sa validation.
            </p>
            @for (item of m.items; track item.id) {
              <div class="done-item">
                {{ item.name }}
                @if (
                  item.eaten_ratio !== null &&
                  item.eaten_ratio !== undefined &&
                  item.eaten_ratio < 1000
                ) {
                  <span class="muted">· mangé aux {{ percent(item.eaten_ratio) }} %</span>
                }
              </div>
            }
            <div class="actions top">
              <button mat-stroked-button (click)="unvalidate(m)" [disabled]="busy()">
                <mat-icon>undo</mat-icon>Annuler la validation
              </button>
            </div>
          </section>
        } @else {
          <form (ngSubmit)="save(m)">
            <section class="card">
              <mat-button-toggle-group
                name="status"
                [ngModel]="status()"
                (ngModelChange)="status.set($event)"
                aria-label="Statut du repas"
              >
                @for (s of statuses; track s) {
                  <mat-button-toggle [value]="s">{{ statusLabels[s] }}</mat-button-toggle>
                }
              </mat-button-toggle-group>
            </section>

            <section class="card">
              <h2 class="widget-title">
                Convives <span class="muted">· {{ portions(equivalents()) }} éq. adulte</span>
              </h2>
              <mat-chip-listbox multiple aria-label="Membres présents">
                @for (member of activeMembers(); track member.id) {
                  <mat-chip-option
                    [selected]="present().has(member.id)"
                    (selectionChange)="toggle(member.id, $event.selected)"
                    >{{ member.name }}</mat-chip-option
                  >
                }
              </mat-chip-listbox>
              @for (g of guests(); track g.key; let i = $index) {
                <div class="row guest">
                  <mat-form-field>
                    <mat-label>Invité</mat-label>
                    <input
                      matInput
                      [name]="'gl' + g.key"
                      [ngModel]="g.label"
                      (ngModelChange)="patchGuest(i, { label: $event })"
                    />
                  </mat-form-field>
                  <mat-form-field class="short">
                    <mat-label>Équivalent adulte</mat-label>
                    <input
                      matInput
                      inputmode="decimal"
                      [name]="'gc' + g.key"
                      [ngModel]="g.coefficient"
                      (ngModelChange)="patchGuest(i, { coefficient: $event })"
                    />
                  </mat-form-field>
                  <button
                    mat-icon-button
                    type="button"
                    aria-label="Retirer l'invité"
                    (click)="removeGuest(i)"
                  >
                    <mat-icon>close</mat-icon>
                  </button>
                </div>
              }
              <button mat-button type="button" (click)="addGuest()">
                <mat-icon>person_add</mat-icon>Invité
              </button>
            </section>

            @if (status() === 'PLANNED') {
              <section class="card">
                <h2 class="widget-title">Au menu</h2>
                @if (m.active_minutes) {
                  <p class="muted times">
                    {{ minutes(m.active_minutes) }} actif, {{ minutes(m.total_minutes) }} au total
                  </p>
                }
                <div class="items">
                  @for (r of rows(); track r.key; let i = $index) {
                    <div class="item">
                      <div class="item-head">
                        <span class="name">{{ r.name }}</span>
                        <button
                          mat-icon-button
                          type="button"
                          aria-label="Retirer l'élément"
                          (click)="removeRow(i)"
                        >
                          <mat-icon>delete</mat-icon>
                        </button>
                      </div>
                      <div class="row">
                        @if (roles().length) {
                          <mat-form-field class="short">
                            <mat-label>Rôle</mat-label>
                            <mat-select
                              [name]="'ro' + r.key"
                              [ngModel]="r.role_id"
                              (ngModelChange)="patch(i, { role_id: $event })"
                            >
                              <mat-option [value]="null">—</mat-option>
                              @for (role of roles(); track role.id) {
                                <mat-option [value]="role.id">{{ role.name }}</mat-option>
                              }
                            </mat-select>
                          </mat-form-field>
                        }
                        @switch (r.source) {
                          @case ('COOK') {
                            <mat-form-field class="short">
                              <mat-label>Portions cuisinées</mat-label>
                              <input
                                matInput
                                inputmode="decimal"
                                [name]="'po' + r.key"
                                [ngModel]="r.portions"
                                (ngModelChange)="patch(i, { portions: $event })"
                              />
                            </mat-form-field>
                            @if (r.info) {
                              <span class="muted">{{ r.info }}</span>
                            }
                          }
                          @case ('LOT') {
                            <mat-form-field class="short">
                              <mat-label>Portions servies</mat-label>
                              <input
                                matInput
                                inputmode="decimal"
                                [name]="'pl' + r.key"
                                [ngModel]="r.portions"
                                (ngModelChange)="patch(i, { portions: $event })"
                              />
                            </mat-form-field>
                          }
                          @case ('FOOD') {
                            <mat-form-field class="short">
                              <mat-label>Quantité</mat-label>
                              <input
                                matInput
                                [name]="'am' + r.key"
                                [ngModel]="r.amount"
                                (ngModelChange)="patch(i, { amount: $event })"
                              />
                            </mat-form-field>
                            <mat-checkbox
                              [name]="'pa' + r.key"
                              [ngModel]="r.per_adult"
                              (ngModelChange)="patch(i, { per_adult: $event })"
                              >par équivalent adulte</mat-checkbox
                            >
                          }
                        }
                      </div>
                    </div>
                  } @empty {
                    <p class="muted">Rien de prévu pour l'instant.</p>
                  }
                </div>

                <div class="add-row">
                  <mat-form-field class="grow">
                    <mat-label>Ajouter une recette</mat-label>
                    <input
                      matInput
                      name="recipe-search"
                      autocomplete="off"
                      [ngModel]="recipeText()"
                      (ngModelChange)="recipeText.set($event)"
                      [matAutocomplete]="recipesAuto"
                    />
                    <mat-autocomplete
                      #recipesAuto="matAutocomplete"
                      (optionSelected)="addRecipe($event.option.value)"
                    >
                      @for (r of recipeResults.value() ?? []; track r.id) {
                        <mat-option [value]="r">{{ r.name }}</mat-option>
                      }
                    </mat-autocomplete>
                  </mat-form-field>
                  <app-item-picker
                    class="grow"
                    label="Ajouter un aliment"
                    (picked)="addFood($event)"
                  />
                  @if (leftovers().length) {
                    <button mat-stroked-button type="button" [matMenuTriggerFor]="leftoverMenu">
                      <mat-icon>takeout_dining</mat-icon>Restes
                    </button>
                    <mat-menu #leftoverMenu>
                      @for (l of leftovers(); track l.label) {
                        <button mat-menu-item type="button" (click)="addLeftover(l)">
                          {{ l.label }}
                        </button>
                      }
                    </mat-menu>
                  }
                </div>
              </section>
            }

            <section class="card">
              <h2 class="widget-title">Horaires</h2>
              <div class="row">
                <mat-form-field class="short">
                  <mat-label>Temps actif disponible</mat-label>
                  <input
                    matInput
                    type="number"
                    min="0"
                    name="minutes"
                    [ngModel]="availableMinutes()"
                    (ngModelChange)="availableMinutes.set($event ?? 0)"
                  />
                  <span matTextSuffix>min</span>
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Cuisiner à partir de</mat-label>
                  <input
                    matInput
                    type="time"
                    name="start"
                    [ngModel]="earliestStart()"
                    (ngModelChange)="earliestStart.set($event)"
                  />
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Prêt à</mat-label>
                  <input
                    matInput
                    type="time"
                    name="ready"
                    [ngModel]="readyBy()"
                    (ngModelChange)="readyBy.set($event)"
                  />
                </mat-form-field>
              </div>
              <mat-form-field class="full-width">
                <mat-label>Note</mat-label>
                <input matInput name="note" [ngModel]="note()" (ngModelChange)="note.set($event)" />
              </mat-form-field>
            </section>

            @if (error()) {
              <p class="error">{{ error() }}</p>
            }
            <div class="actions bottom">
              <button mat-flat-button type="submit" [disabled]="busy()">Enregistrer</button>
              @if (m.status === 'PLANNED' && m.date <= today) {
                <button mat-stroked-button type="button" (click)="validate(m)" [disabled]="busy()">
                  <mat-icon>task_alt</mat-icon>Valider le repas
                </button>
              }
              <span class="spacer"></span>
              <button mat-button type="button" class="destructive" (click)="remove(m)">
                Supprimer le repas
              </button>
            </div>
          </form>
        }
      } @else {
        <mat-progress-bar mode="indeterminate" />
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .warning {
      background: var(--mat-sys-tertiary-container);
      color: var(--mat-sys-on-tertiary-container);
    }
    .signal {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .guest {
      margin-top: 1rem;
    }
    .short {
      width: 12rem;
    }
    .grow {
      flex: 1;
      min-width: 16rem;
    }
    .times {
      margin: -0.5rem 0 1rem;
    }
    .items {
      display: flex;
      flex-direction: column;
      gap: 1rem;
      margin-bottom: 1rem;
    }
    .item {
      border-bottom: 1px solid var(--mat-sys-outline-variant);
      padding-bottom: 0.75rem;
    }
    .item-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    .item-head .name {
      font: var(--mat-sys-title-small);
    }
    .add-row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .done-item {
      padding: 0.25rem 0;
    }
    .top {
      margin-top: 1rem;
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
export class MealEditor {
  private readonly api = inject(PlanningService);
  private readonly recipes = inject(RecipesService);
  private readonly stock = inject(StockService);
  private readonly router = inject(Router);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly version = inject(DataVersion);

  readonly id = input.required<string>();

  protected readonly today = todayIso();
  protected readonly statuses: MealStatus[] = ['PLANNED', 'SKIPPED', 'OUT_OF_HOME'];
  protected readonly statusLabels = MEAL_STATUS_LABELS;
  protected readonly portions = formatPortions;
  protected readonly dayHeader = formatDayHeader;
  protected readonly week = weekStart;

  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly status = signal<MealStatus>('PLANNED');
  protected readonly availableMinutes = signal(0);
  protected readonly earliestStart = signal('');
  protected readonly readyBy = signal('');
  protected readonly note = signal('');
  protected readonly present = signal(new Set<string>());
  protected readonly guests = signal<Guest[]>([]);
  protected readonly rows = signal<Row[]>([]);
  protected readonly recipeText = signal<string | RecipeSummary>('');

  protected readonly meal = rxResource({
    params: () => ({ id: this.id(), v: this.version.value() }),
    stream: ({ params }) => this.api.getMeal({ id: params.id }),
  });

  private readonly members = rxResource({ stream: () => this.api.listMembers() });
  private readonly template = rxResource({ stream: () => this.api.getWeekTemplate() });

  protected readonly activeMembers = computed(() => {
    const present = this.present();
    return (this.members.value() ?? []).filter((m) => m.active || present.has(m.id));
  });

  protected readonly roles = computed(() => {
    const slot = this.meal.value()?.slot_id;
    return this.template.value()?.find((t) => t.slot_id === slot)?.roles ?? [];
  });

  /** Somme des coefficients des convives (R-PLA-11). */
  protected readonly equivalents = computed(() => {
    const members = this.members.value() ?? [];
    return total([
      ...members.filter((m) => this.present().has(m.id)).map((m) => m.adult_equivalent),
      ...this.guests().map((g) => parsePortions(g.coefficient) ?? 0),
    ]);
  });

  private readonly recipeQuery = toSignal(toObservable(this.recipeText).pipe(debounceTime(200)), {
    initialValue: '' as string | RecipeSummary,
  });
  protected readonly recipeResults = rxResource({
    params: () => {
      const q = this.recipeQuery();
      return typeof q === 'string' ? q.trim() : '';
    },
    stream: ({ params: q }) =>
      q.length < 2 ? of([]) : this.recipes.searchRecipes({ q }).pipe(map((r) => r.slice(0, 10))),
  });

  /** Restes prévus des repas des 7 jours précédents, et restes en stock (ADR-0012). */
  private readonly previous = rxResource({
    params: () => this.meal.value()?.date,
    stream: ({ params: date }) =>
      date ? this.api.getPlan({ start: addDays(date, -6), days: 7 }) : of(null),
  });
  private readonly leftoverLots = rxResource({
    params: () => this.version.value(),
    stream: () => this.stock.listLots().pipe(map((lots) => lots.filter((l) => l.recipe_id))),
  });

  protected readonly leftovers = computed<LeftoverChoice[]>(() => {
    const meal = this.meal.value();
    const plan = this.previous.value();
    if (!meal || !plan) return [];
    const order = (slot: string) => plan.slots.findIndex((s) => s.id === slot);
    const earlier = (m: Meal) =>
      m.date < meal.date || (m.date === meal.date && order(m.slot_id) < order(meal.slot_id));
    const used = new Set(this.rows().map((r) => r.leftover_of_item_id ?? r.lot_id));
    const planned = plan.days
      .flatMap((d) => d.meals)
      .filter((m) => m.id !== meal.id && m.status === 'PLANNED' && earlier(m))
      .flatMap((m) =>
        m.items
          .filter(
            (i) => i.source === 'COOK' && (i.planned_leftover ?? 0) > 0 && !used.has(i.id ?? ''),
          )
          .map((i) => ({
            label: `${i.name} (${m.slot_name.toLowerCase()} du ${formatShortDate(m.date)}) — ${formatPortions(i.planned_leftover)} portion(s) prévues`,
            name: `Restes : ${i.name}`,
            portions: i.planned_leftover ?? 0,
            leftover_of_item_id: i.id ?? null,
            lot_id: null,
          })),
      );
    const stocked = (this.leftoverLots.value() ?? [])
      .filter((l) => !used.has(l.id))
      .map((l) => ({
        label: `${l.name} — ${formatPortions(l.quantity)} portion(s) en stock`,
        name: l.name,
        portions: l.quantity ?? 0,
        leftover_of_item_id: null,
        lot_id: l.id,
      }));
    return [...planned, ...stocked];
  });

  constructor() {
    effect(() => {
      const m = this.meal.value();
      if (!m) return;
      untracked(() => this.load(m));
    });
  }

  private load(m: Meal): void {
    this.status.set(m.status === 'DONE' ? 'PLANNED' : m.status);
    this.availableMinutes.set(m.available_active_minutes);
    this.earliestStart.set(m.earliest_start);
    this.readyBy.set(m.ready_by);
    this.note.set(m.note ?? '');
    this.present.set(
      new Set(m.diners.filter((d) => d.member_id).map((d) => d.member_id as string)),
    );
    this.guests.set(
      m.diners
        .filter((d) => !d.member_id)
        .map((d) => ({
          key: keys++,
          label: d.guest_label ?? '',
          coefficient: formatPortions(d.adult_equivalent),
        })),
    );
    this.rows.set(m.items.map((i) => this.row(i)));
  }

  private row(i: MealItem): Row {
    const deadline = i.leftover_deadline ? formatShortDate(i.leftover_deadline) : '';
    return {
      key: keys++,
      id: i.id ?? null,
      role_id: i.role_id ?? null,
      source: i.source,
      name: i.name ?? '',
      recipe_id: i.recipe_id ?? null,
      portions:
        i.source === 'COOK'
          ? formatPortions(i.cooked_portions)
          : i.source === 'LOT'
            ? formatPortions(i.amount)
            : '',
      lot_id: i.lot_id ?? null,
      leftover_of_item_id: i.leftover_of_item_id ?? null,
      food_id: i.food_id ?? null,
      product_id: i.product_id ?? null,
      amount:
        i.source === 'FOOD' && i.amount && i.measure ? quantityInput(i.amount, i.measure) : '',
      measure: i.measure ?? null,
      per_adult: i.per_adult_equivalent ?? false,
      info:
        i.planned_leftover && i.planned_leftover > 0
          ? `Reste prévu : ${formatPortions(i.planned_leftover)} portion(s), à manger d'ici le ${deadline}`
          : '',
    };
  }

  protected text(s: Meal['signals'][number]): string {
    const name = (id: string) =>
      this.members.value()?.find((m) => m.id === id)?.name ?? 'Un convive';
    return signalText(s, formatPortions, name);
  }

  protected minutes(value: number): string {
    return value >= 60
      ? `${Math.floor(value / 60)} h ${String(value % 60).padStart(2, '0')}`
      : `${value} min`;
  }

  protected readonly percent = percent;

  protected toggle(member: string, selected: boolean): void {
    this.present.update((set) => {
      const next = new Set(set);
      if (selected) next.add(member);
      else next.delete(member);
      return next;
    });
  }

  protected addGuest(): void {
    this.guests.update((g) => [...g, { key: keys++, label: '', coefficient: '1' }]);
  }

  protected patchGuest(index: number, change: Partial<Guest>): void {
    this.guests.update((g) => g.map((x, i) => (i === index ? { ...x, ...change } : x)));
  }

  protected removeGuest(index: number): void {
    this.guests.update((g) => g.filter((_, i) => i !== index));
  }

  protected patch(index: number, change: Partial<Row>): void {
    this.rows.update((r) => r.map((x, i) => (i === index ? { ...x, ...change } : x)));
  }

  protected removeRow(index: number): void {
    this.rows.update((r) => r.filter((_, i) => i !== index));
  }

  private emptyRow(source: ItemSource, name: string): Row {
    return {
      key: keys++,
      id: null,
      role_id: null,
      source,
      name,
      recipe_id: null,
      portions: '',
      lot_id: null,
      leftover_of_item_id: null,
      food_id: null,
      product_id: null,
      amount: '',
      measure: null,
      per_adult: false,
      info: '',
    };
  }

  /** Une recette cuisinée, pour les convives du repas par défaut (R-PLA-12) ; au premier rôle libre admis. */
  protected addRecipe(recipe: RecipeSummary): void {
    const taken = new Set(this.rows().map((r) => r.role_id));
    const role = this.roles().find(
      (r) => !taken.has(r.id ?? null) && (r.recipe_kinds ?? []).includes(recipe.kind),
    );
    this.rows.update((r) => [
      ...r,
      {
        ...this.emptyRow('COOK', recipe.name),
        recipe_id: recipe.id,
        role_id: role?.id ?? null,
        portions: formatPortions(this.equivalents() || 1000),
      },
    ]);
    this.recipeText.set('');
  }

  protected addFood(item: StockItem): void {
    const measure =
      item.kind === 'food' ? item.food.effective.stock_measure : item.product.stock_measure;
    this.rows.update((r) => [
      ...r,
      {
        ...this.emptyRow('FOOD', itemName(item)),
        food_id: item.kind === 'food' ? item.food.id : null,
        product_id: item.kind === 'product' ? item.product.id : null,
        measure,
        amount: '',
      },
    ]);
  }

  protected addLeftover(choice: LeftoverChoice): void {
    const wanted = smallest(choice.portions, this.equivalents() || choice.portions);
    // Un reste de plat tient le rôle du plat : le premier rôle obligatoire encore libre.
    const taken = new Set(this.rows().map((r) => r.role_id));
    const role = this.roles().find((r) => r.required && !taken.has(r.id ?? null));
    this.rows.update((r) => [
      ...r,
      {
        ...this.emptyRow('LOT', choice.name),
        role_id: role?.id ?? null,
        lot_id: choice.lot_id,
        leftover_of_item_id: choice.leftover_of_item_id,
        portions: formatPortions(wanted),
      },
    ]);
  }

  /** Éléments saisis → API ; une saisie invalide renvoie son message. */
  private items(): MealItem[] | string {
    const out: MealItem[] = [];
    for (const r of this.rows()) {
      const base: MealItem = { id: r.id, role_id: r.role_id, source: r.source };
      if (r.source === 'COOK') {
        const portions = parsePortions(r.portions);
        if (!portions) return `Portions cuisinées invalides pour ${r.name}.`;
        out.push({ ...base, recipe_id: r.recipe_id, cooked_portions: portions });
      } else if (r.source === 'LOT') {
        const portions = parsePortions(r.portions);
        if (!portions) return `Portions servies invalides pour ${r.name}.`;
        out.push({
          ...base,
          lot_id: r.lot_id,
          leftover_of_item_id: r.leftover_of_item_id,
          amount: portions,
        });
      } else {
        const amount = r.measure ? parseQuantity(r.amount, r.measure) : null;
        if (!amount) {
          const unit = r.measure ? formatQuantity(1000, r.measure).split('\u00a0')[1] : '';
          return `Quantité invalide pour ${r.name} (en ${unit}).`;
        }
        out.push({
          ...base,
          food_id: r.food_id,
          product_id: r.product_id,
          amount,
          per_adult_equivalent: r.per_adult,
        });
      }
    }
    return out;
  }

  protected async save(m: Meal): Promise<void> {
    const items = this.status() === 'PLANNED' ? this.items() : [];
    if (typeof items === 'string') {
      this.error.set(items);
      return;
    }
    const members = this.members.value() ?? [];
    const diners = [
      ...members
        .filter((x) => this.present().has(x.id))
        .map((x) => ({ member_id: x.id, adult_equivalent: x.adult_equivalent })),
      ...this.guests().map((g) => ({
        guest_label: g.label || null,
        adult_equivalent: parsePortions(g.coefficient) ?? 1000,
      })),
    ];
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(
        this.api.updateMeal({
          id: m.id,
          body: {
            status: this.status(),
            available_active_minutes: Number(this.availableMinutes()) || 0,
            earliest_start: this.earliestStart(),
            ready_by: this.readyBy(),
            locked: m.locked,
            note: this.note() || null,
            diners,
            items: this.status() === 'PLANNED' ? items : m.items.map((i) => ({ ...i })),
          },
        }),
      );
      this.snack.open('Repas enregistré', undefined, { duration: 2500 });
      this.version.bump();
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }

  protected async validate(m: Meal): Promise<void> {
    await this.save(m);
    if (!this.error()) {
      await this.router.navigate(['/repas', m.id, 'validation']);
    }
  }

  protected async unvalidate(m: Meal): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Annuler la validation',
      message: 'Les sorties de stock et les restes de ce repas sont annulés ; il redevient prévu.',
      action: 'Annuler la validation',
      destructive: true,
    });
    if (!ok) return;
    await this.run(
      () => firstValueFrom(this.api.unvalidateMeal({ id: m.id })),
      'Validation annulée',
    );
  }

  protected async remove(m: Meal): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le repas',
      message:
        'Le repas et ses éléments sont supprimés, ainsi que les restes prévus qu’il fournissait.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    if (await this.run(() => firstValueFrom(this.api.deleteMeal({ id: m.id })), 'Repas supprimé')) {
      await this.router.navigate(['/repas'], { queryParams: { semaine: weekStart(m.date) } });
    }
  }

  private async run(call: () => Promise<unknown>, message: string): Promise<boolean> {
    this.busy.set(true);
    try {
      await call();
      this.snack.open(message, undefined, { duration: 2500 });
      this.version.bump();
      return true;
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
      return false;
    } finally {
      this.busy.set(false);
    }
  }
}
