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
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { LineInput } from '../../api/models/line-input';
import { LineTarget } from '../../api/models/line-target';
import { Measure } from '../../api/models/measure';
import { ValidationProposal } from '../../api/models/validation-proposal';
import { PlanningService } from '../../api/services/planning.service';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { formatDayHeader, formatShortDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import {
  formatPortions,
  formatQuantity,
  parsePortions,
  parseQuantity,
  quantityInput,
  remainder,
  total,
} from '../../core/quantity';

/** Une prise dans un lot, en cours de saisie. */
interface TakeRow {
  lot_id: string;
  name: string;
  text: string;
  expiry: string | null;
}

/** Un besoin d'un élément : ce qui est utilisé et les lots où on le prend. */
interface LineRow {
  key: string;
  item_id: string;
  target: LineTarget;
  name: string;
  measure: Measure | null;
  used: string;
  takes: TakeRow[];
  /** Lots à date dépassée, à vérifier dans le stock (R-STK-37). */
  to_confirm: string[];
}

interface LeftoverRow {
  item_id: string;
  name: string;
  portions: string;
  location_id: string | null;
  as_food: boolean;
  food_name: string | null;
  freezable: boolean;
}

/**
 * Validation d'un repas (PLA-04, ADR-0005) : les sorties proposées, ajustables (ce qui a été utilisé, les lots où
 * on l'a pris), et les restes de chaque recette cuisinée (ADR-0012).
 */
@Component({
  selector: 'app-meal-validation',
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
        <a mat-icon-button [routerLink]="['/repas', id()]" aria-label="Retour au repas"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Valider le repas</h1>
      </div>
      @if (proposal.value(); as p) {
        <p class="muted">
          {{ dayHeader(p.date) }} · les quantités sont proposées d'après les portions cuisinées et
          le stock, la date la plus proche d'abord. Corrigez ce qui a été réellement utilisé.
        </p>
        <form (ngSubmit)="submit(p)">
          @for (item of p.items; track item.item_id) {
            <section class="card">
              <h2 class="widget-title">
                {{ item.name }}
                @if (item.cooked_portions) {
                  <span class="muted"
                    >· cuisiné pour {{ portions(item.cooked_portions) }} portions</span
                  >
                }
              </h2>
              @for (l of linesOf(item.item_id); track l.key) {
                <div class="line">
                  <div class="line-head">
                    <span class="name">{{ l.name }}</span>
                    <mat-form-field class="short">
                      <mat-label>Utilisé</mat-label>
                      <input
                        matInput
                        [name]="'u' + l.key"
                        [ngModel]="l.used"
                        (ngModelChange)="patchLine(l.key, { used: $event })"
                      />
                    </mat-form-field>
                  </div>
                  @for (t of l.takes; track t.lot_id; let ti = $index) {
                    <div class="take">
                      <span>
                        <mat-icon>inventory_2</mat-icon>{{ t.name }}
                        @if (t.expiry) {
                          <span class="muted">· {{ shortDate(t.expiry) }}</span>
                        }
                      </span>
                      <mat-form-field class="short">
                        <mat-label>Sorti du lot</mat-label>
                        <input
                          matInput
                          [name]="'t' + l.key + ti"
                          [ngModel]="t.text"
                          (ngModelChange)="patchTake(l.key, ti, $event)"
                        />
                      </mat-form-field>
                    </div>
                  }
                  @if (uncovered(l); as rest) {
                    <div class="muted note">
                      {{ rest }} sans sortie de stock (aliment absent ou non suivi).
                    </div>
                  }
                  @for (c of l.to_confirm; track c) {
                    <div class="status expired">
                      {{ c }} : date dépassée, à vérifier dans le stock
                    </div>
                  }
                </div>
              }
              @if (item.unquantified.length) {
                <p class="muted">Utilisés sans quantité : {{ item.unquantified.join(', ') }}.</p>
              }
              @if (leftoverOf(item.item_id); as left) {
                <div class="leftover">
                  <h3>Restes</h3>
                  <div class="row">
                    <mat-form-field class="short">
                      <mat-label>{{
                        left.as_food ? 'Quantité de reste (g)' : 'Portions restantes'
                      }}</mat-label>
                      <input
                        matInput
                        [name]="'lp' + item.item_id"
                        [ngModel]="left.portions"
                        (ngModelChange)="patchLeftover(item.item_id, { portions: $event })"
                      />
                    </mat-form-field>
                    <mat-form-field>
                      <mat-label>Rangés</mat-label>
                      <mat-select
                        [name]="'ll' + item.item_id"
                        [ngModel]="left.location_id"
                        (ngModelChange)="patchLeftover(item.item_id, { location_id: $event })"
                      >
                        @for (loc of locations(); track loc.id) {
                          @if (loc.kind !== 'FREEZER' || left.freezable) {
                            <mat-option [value]="loc.id">{{ loc.name }}</mat-option>
                          }
                        }
                      </mat-select>
                    </mat-form-field>
                    @if (left.food_name) {
                      <mat-checkbox
                        [name]="'la' + item.item_id"
                        [ngModel]="left.as_food"
                        (ngModelChange)="patchLeftover(item.item_id, { as_food: $event })"
                        >Comme « {{ left.food_name }} »</mat-checkbox
                      >
                    }
                  </div>
                </div>
              }
            </section>
          } @empty {
            <section class="card"><p>Rien à sortir du stock pour ce repas.</p></section>
          }
          @if (error()) {
            <p class="error">{{ error() }}</p>
          }
          <div class="actions bottom">
            <button mat-flat-button type="submit" [disabled]="busy()">
              <mat-icon>task_alt</mat-icon>Valider
            </button>
            <a mat-button [routerLink]="['/repas', id()]">Annuler</a>
          </div>
        </form>
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
    .line {
      border-bottom: 1px solid var(--mat-sys-outline-variant);
      padding: 0.75rem 0;
    }
    .line-head,
    .take {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      justify-content: space-between;
      gap: 0.5rem 1rem;
    }
    .line-head .name {
      font: var(--mat-sys-title-small);
    }
    .take {
      padding-left: 1rem;
    }
    .take mat-icon {
      vertical-align: middle;
      margin-right: 0.25rem;
      font-size: 1.125rem;
      width: 1.125rem;
      height: 1.125rem;
    }
    .short {
      width: 11rem;
    }
    .note {
      padding-left: 1rem;
    }
    .leftover h3 {
      font: var(--mat-sys-title-small);
      margin: 1rem 0 0.5rem;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .bottom {
      margin-bottom: 2rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class MealValidation {
  private readonly api = inject(PlanningService);
  private readonly stock = inject(StockService);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  private readonly version = inject(DataVersion);

  readonly id = input.required<string>();

  protected readonly portions = formatPortions;
  protected readonly dayHeader = formatDayHeader;
  protected readonly shortDate = formatShortDate;
  protected readonly message = errorMessage;
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly lines = signal<LineRow[]>([]);
  protected readonly leftovers = signal<LeftoverRow[]>([]);

  protected readonly proposal = rxResource({
    params: () => this.id(),
    stream: ({ params: id }) => this.api.getMealValidation({ id }),
  });
  private readonly locationsResource = rxResource({ stream: () => this.stock.listLocations() });
  protected readonly locations = computed(() => this.locationsResource.value() ?? []);

  constructor() {
    effect(() => {
      const p = this.proposal.value();
      if (p) untracked(() => this.load(p));
    });
  }

  private load(p: ValidationProposal): void {
    const amount = (raw: number, measure: Measure | null | undefined) =>
      measure ? quantityInput(raw, measure) : formatPortions(raw);
    this.lines.set(
      p.items.flatMap((item) =>
        item.lines.map((l, index) => ({
          key: `${item.item_id}-${index}`,
          item_id: item.item_id,
          target: l.target,
          name: l.name,
          measure: l.measure ?? null,
          used: amount(l.needed, l.measure),
          takes: l.takes.map((t) => ({
            lot_id: t.lot_id,
            name: t.name,
            text: amount(t.amount, l.measure),
            expiry: t.expiry ?? null,
          })),
          to_confirm: l.to_confirm.map((t) => t.name),
        })),
      ),
    );
    this.leftovers.set(
      p.items
        .filter((i) => i.source === 'COOK')
        .map((i) => ({
          item_id: i.item_id,
          name: i.name,
          portions: i.planned_leftover ? formatPortions(i.planned_leftover) : '0',
          location_id: p.leftover_location_id ?? null,
          as_food: false,
          food_name: i.leftover_food_name ?? null,
          freezable: i.freezable,
        })),
    );
  }

  protected linesOf(item: string): LineRow[] {
    return this.lines().filter((l) => l.item_id === item);
  }

  protected leftoverOf(item: string): LeftoverRow | undefined {
    return this.leftovers().find((l) => l.item_id === item);
  }

  private parse(text: string, measure: Measure | null): number | null {
    return measure ? parseQuantity(text, measure) : parsePortions(text);
  }

  /** Ce qui est utilisé sans sortie de stock (R-PLA-32), mis en forme ; vide si tout sort d'un lot. */
  protected uncovered(line: LineRow): string {
    const used = this.parse(line.used, line.measure) ?? 0;
    const taken = total(line.takes.map((t) => this.parse(t.text, line.measure) ?? 0));
    const rest = remainder(used, taken);
    if (!rest) return '';
    return line.measure ? formatQuantity(rest, line.measure) : `${formatPortions(rest)} portion(s)`;
  }

  protected patchLine(key: string, change: Partial<LineRow>): void {
    this.lines.update((rows) => rows.map((r) => (r.key === key ? { ...r, ...change } : r)));
  }

  protected patchTake(key: string, index: number, text: string): void {
    this.lines.update((rows) =>
      rows.map((r) =>
        r.key === key
          ? { ...r, takes: r.takes.map((t, i) => (i === index ? { ...t, text } : t)) }
          : r,
      ),
    );
  }

  protected patchLeftover(item: string, change: Partial<LeftoverRow>): void {
    this.leftovers.update((rows) =>
      rows.map((r) => (r.item_id === item ? { ...r, ...change } : r)),
    );
  }

  protected async submit(p: ValidationProposal): Promise<void> {
    const lines: LineInput[] = [];
    for (const l of this.lines()) {
      const used = this.parse(l.used, l.measure);
      if (used === null) {
        this.error.set(`Quantité utilisée invalide pour ${l.name}.`);
        return;
      }
      const takes = [];
      for (const t of l.takes) {
        const amount = this.parse(t.text, l.measure);
        if (amount === null) {
          this.error.set(`Quantité invalide pour le lot ${t.name}.`);
          return;
        }
        if (amount > 0) takes.push({ lot_id: t.lot_id, amount });
      }
      lines.push({ item_id: l.item_id, target: l.target, measure: l.measure, used, takes });
    }
    const leftovers = [];
    for (const left of this.leftovers()) {
      // Un reste en aliment se saisit en masse (« 300 g ») ; un reste de la recette, en portions.
      const amount = left.as_food
        ? parseQuantity(left.portions, 'MASS')
        : parsePortions(left.portions);
      if (amount === null) {
        this.error.set(`Reste invalide pour ${left.name}.`);
        return;
      }
      if (amount > 0) {
        if (!left.location_id) {
          this.error.set(`Choisissez où ranger les restes de ${left.name}.`);
          return;
        }
        leftovers.push({
          item_id: left.item_id,
          amount,
          location_id: left.location_id,
          as_food: left.as_food,
        });
      }
    }
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(this.api.validateMeal({ id: p.meal_id, body: { lines, leftovers } }));
      this.version.bump();
      this.snack.open('Repas validé', undefined, { duration: 3000 });
      await this.router.navigate(['/repas', p.meal_id]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
