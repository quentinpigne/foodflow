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
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Router, RouterLink } from '@angular/router';
import { Observable, firstValueFrom } from 'rxjs';

import { FoodDetail } from '../../api/models/food-detail';
import { Measure } from '../../api/models/measure';
import { NutrientBasis } from '../../api/models/nutrient-basis';
import { Product } from '../../api/models/product';
import { Settings } from '../../api/models/settings';
import { FoodsService } from '../../api/services/foods.service';
import { StockService } from '../../api/services/stock.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { MEASURE_LABELS, SOURCE_LABELS, TRACKING_LABELS } from '../../core/labels';
import { NUTRIENTS, formatNutrient, parseNutrient } from '../../core/nutrients';
import { UNITS, defaultUnit, formatIn, formatQuantity, parseQuantity } from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { LotRow } from '../../shared/lot-row';
import { SettingsEditor } from '../../shared/settings-editor';
import { GroupsStore } from '../../shared/stores';
import { AddLot } from '../stock/add-lot';
import { ProductDialog } from './product-dialog';
import { TrackingDialog } from './tracking-dialog';

/** Fiche d'un aliment (ALI-02, ALI-04, ALI-05) : réglages, conversions, valeurs, unités, produits, stock. */
@Component({
  selector: 'app-food-page',
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
    LotRow,
    SettingsEditor,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/aliments" aria-label="Retour aux aliments"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>{{ food.value()?.name ?? 'Aliment' }}</h1>
        @if (food.hasValue()) {
          <button
            mat-stroked-button
            (click)="addLot.open({ item: { kind: 'food', food: food.value() } })"
          >
            <mat-icon>add</mat-icon>Ajouter au stock
          </button>
        }
      </div>
      @if (food.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (food.hasValue()) {
        @let f = food.value();
        <section class="card">
          <h2 class="widget-title">Général</h2>
          <form class="stack" (ngSubmit)="saveGeneral(f)">
            <div class="form-grid">
              <mat-form-field>
                <mat-label>Nom</mat-label>
                <input
                  matInput
                  name="name"
                  [ngModel]="name()"
                  (ngModelChange)="name.set($event)"
                  required
                />
              </mat-form-field>
              <mat-form-field>
                <mat-label>Famille</mat-label>
                <mat-select name="group" [ngModel]="group()" (ngModelChange)="group.set($event)">
                  @for (g of groups.list(); track g.id) {
                    <mat-option [value]="g.id">{{ g.path }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Alias</mat-label>
                <input
                  matInput
                  name="aliases"
                  [ngModel]="aliases()"
                  (ngModelChange)="aliases.set($event)"
                />
                <mat-hint>Séparés par des virgules</mat-hint>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Densité</mat-label>
                <input
                  matInput
                  name="density"
                  inputmode="decimal"
                  [ngModel]="density()"
                  (ngModelChange)="density.set($event)"
                />
                <span matTextSuffix>g/mL</span>
              </mat-form-field>
              <mat-form-field>
                <mat-label>Poids d'une pièce</mat-label>
                <input
                  matInput
                  name="piece"
                  inputmode="decimal"
                  [ngModel]="piece()"
                  (ngModelChange)="piece.set($event)"
                />
                <span matTextSuffix>g</span>
              </mat-form-field>
            </div>
            <app-settings-editor
              [(settings)]="settings"
              [inherited]="inherited()"
              [lockTracking]="hasLots()"
            />
            <mat-checkbox name="hidden" [ngModel]="hidden()" (ngModelChange)="hidden.set($event)"
              >Masqué dans les listes</mat-checkbox
            >
            <div class="actions">
              <button mat-flat-button type="submit" [disabled]="busy()">Enregistrer</button>
              @if (hasLots()) {
                <button mat-stroked-button type="button" (click)="changeTracking(f)">
                  Mode de suivi : {{ trackingLabels[f.effective.tracking_mode] }}
                </button>
              }
              <span class="spacer"></span>
              <button mat-button type="button" class="danger" (click)="remove(f)">Supprimer</button>
            </div>
          </form>
        </section>

        <section class="card">
          <h2 class="widget-title">Valeurs nutritionnelles</h2>
          <p class="muted">
            Pour 100 {{ basis() === 'PER_100_ML' ? 'mL' : 'g' }}. Une case vide est une valeur
            inconnue, jamais un zéro.
          </p>
          <form (ngSubmit)="saveNutrients(f)">
            <mat-form-field>
              <mat-label>Base</mat-label>
              <mat-select name="basis" [ngModel]="basis()" (ngModelChange)="basis.set($event)">
                <mat-option value="PER_100_G">Pour 100 g</mat-option>
                <mat-option value="PER_100_ML">Pour 100 mL</mat-option>
              </mat-select>
            </mat-form-field>
            <div class="nutrients">
              @for (n of nutrientInfo; track n.code) {
                @let current = nutrientOf(f, n.code);
                <div class="nutrient">
                  <span class="label">{{ n.label }}</span>
                  <mat-form-field class="value">
                    <input
                      matInput
                      [name]="n.code"
                      inputmode="decimal"
                      [ngModel]="values()[n.code] ?? ''"
                      (ngModelChange)="setValue(n.code, $event)"
                    />
                    <span matTextSuffix>{{ n.unit }}</span>
                  </mat-form-field>
                  <span class="source muted">
                    @if (current) {
                      {{ sourceLabels[current.source] }}{{ current.estimated ? ', estimée' : '' }}
                    }
                    @if ((current?.reference_value ?? null) !== null) {
                      <button mat-button type="button" (click)="restore(f, n.code)">
                        Rétablir Ciqual
                      </button>
                    }
                  </span>
                </div>
              }
            </div>
            <button mat-flat-button type="submit" [disabled]="busy()">
              Enregistrer les valeurs
            </button>
          </form>
        </section>

        <section class="card">
          <h2 class="widget-title">Unités propres</h2>
          <p class="muted">Gousse, sachet, tranche… : leur valeur dans une mesure.</p>
          @for (u of f.units; track u.id) {
            <div class="line">
              <span>1 {{ u.name }} = {{ formatQuantity(u.amount, u.measure) }}</span>
              <button mat-icon-button aria-label="Supprimer l'unité" (click)="deleteUnit(f, u.id)">
                <mat-icon>delete</mat-icon>
              </button>
            </div>
          }
          <form class="row" (ngSubmit)="addUnit(f)">
            <mat-form-field>
              <mat-label>Nom</mat-label>
              <input
                matInput
                name="unitName"
                [ngModel]="unitName()"
                (ngModelChange)="unitName.set($event)"
              />
            </mat-form-field>
            <mat-form-field>
              <mat-label>Mesure</mat-label>
              <mat-select
                name="unitMeasure"
                [ngModel]="unitMeasure()"
                (ngModelChange)="unitMeasure.set($event)"
              >
                @for (m of measures; track m) {
                  <mat-option [value]="m">{{ measureLabels[m] }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Valeur</mat-label>
              <input
                matInput
                name="unitAmount"
                inputmode="decimal"
                [ngModel]="unitAmount()"
                (ngModelChange)="unitAmount.set($event)"
              />
              <span matTextSuffix>{{ unitSymbol() }}</span>
            </mat-form-field>
            <button mat-stroked-button type="submit" [disabled]="!unitName().trim()">
              Ajouter
            </button>
          </form>
        </section>

        <section class="card">
          <h2 class="widget-title">Produits</h2>
          @for (p of f.products; track p.id) {
            <div class="line">
              <button
                mat-icon-button
                [attr.aria-label]="'Produit habituel : ' + p.name"
                (click)="prefer(f, p)"
              >
                <mat-icon>{{ f.preferred_product_id === p.id ? 'star' : 'star_border' }}</mat-icon>
              </button>
              <span class="grow">
                {{ p.name }}
                <span class="muted">
                  @if (p.brand) {
                    · {{ p.brand }}
                  }
                  @if (p.package_amount) {
                    · {{ formatQuantity(p.package_amount, p.stock_measure) }}
                  }
                  @if (p.barcode) {
                    · {{ p.barcode }}
                  }
                </span>
              </span>
              <button mat-icon-button aria-label="Modifier le produit" (click)="editProduct(f, p)">
                <mat-icon>edit</mat-icon>
              </button>
            </div>
          }
          <button mat-stroked-button (click)="editProduct(f)">
            <mat-icon>add</mat-icon>Ajouter un produit
          </button>
        </section>

        @if (lots.value()?.length) {
          <section class="card">
            <h2 class="widget-title">En stock</h2>
            <div class="lot-list">
              @for (lot of lots.value(); track lot.id) {
                <app-lot-row [lot]="lot" />
              }
            </div>
          </section>
        }
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .stack {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .actions {
      align-items: center;
    }
    .spacer,
    .grow {
      flex: 1;
    }
    .danger {
      color: var(--mat-sys-error);
    }
    .nutrients {
      display: flex;
      flex-direction: column;
      gap: 0.25rem;
      margin-bottom: 1rem;
    }
    .nutrient {
      display: grid;
      grid-template-columns: minmax(10rem, 14rem) 9rem 1fr;
      align-items: center;
      gap: 0.75rem;
    }
    .source {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .line {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin-top: 0.5rem;
    }
    @media (max-width: 599.98px) {
      .nutrient {
        grid-template-columns: 1fr 8rem;
      }
      .source {
        grid-column: 1 / -1;
      }
    }
  `,
})
export class FoodPage {
  private readonly api = inject(FoodsService);
  private readonly stock = inject(StockService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly router = inject(Router);
  protected readonly groups = inject(GroupsStore);
  protected readonly addLot = inject(AddLot);

  readonly id = input.required<string>();

  protected readonly nutrientInfo = NUTRIENTS;
  protected readonly sourceLabels = SOURCE_LABELS;
  protected readonly trackingLabels = TRACKING_LABELS;
  protected readonly measureLabels = MEASURE_LABELS;
  protected readonly measures: Measure[] = ['MASS', 'VOLUME', 'COUNT'];
  protected readonly formatQuantity = formatQuantity;

  protected readonly food = rxResource({
    params: () => ({ id: this.id(), v: this.version.value() }),
    stream: ({ params }) => this.api.getFood({ id: params.id }),
  });
  protected readonly lots = rxResource({
    params: () => ({ id: this.id(), v: this.version.value() }),
    stream: ({ params }) => this.stock.listLots({ food_id: params.id }),
  });

  protected readonly name = signal('');
  protected readonly group = signal('');
  protected readonly aliases = signal('');
  protected readonly density = signal('');
  protected readonly piece = signal('');
  protected readonly hidden = signal(false);
  protected readonly settings = signal<Settings>({});
  protected readonly basis = signal<NutrientBasis>('PER_100_G');
  protected readonly values = signal<Record<string, string>>({});
  protected readonly unitName = signal('');
  protected readonly unitMeasure = signal<Measure>('MASS');
  protected readonly unitAmount = signal('');
  protected readonly busy = signal(false);

  protected readonly hasLots = computed(() => (this.lots.value()?.length ?? 0) > 0);
  protected readonly inherited = computed(
    () => this.groups.list().find((g) => g.id === this.group())?.effective ?? null,
  );
  protected readonly unitSymbol = computed(() => defaultUnit(this.unitMeasure()).symbol);

  constructor() {
    effect(() => {
      if (!this.food.hasValue()) return;
      const f = this.food.value();
      this.name.set(f.name);
      this.group.set(f.group_id);
      this.aliases.set(f.aliases.join(', '));
      this.density.set(f.density_mg_per_ml ? formatNutrient(f.density_mg_per_ml) : '');
      this.piece.set(f.piece_weight_mg ? formatIn(f.piece_weight_mg, UNITS.MASS[0]) : '');
      this.hidden.set(f.hidden);
      this.settings.set({ ...f.settings });
      this.basis.set(f.nutrient_basis);
      this.values.set(
        Object.fromEntries(f.nutrients.map((n) => [n.code, formatNutrient(n.value)])),
      );
    });
  }

  protected nutrientOf(food: FoodDetail, code: string) {
    return food.nutrients.find((n) => n.code === code) ?? null;
  }

  protected setValue(code: string, value: string): void {
    this.values.update((v) => ({ ...v, [code]: value }));
  }

  protected async saveGeneral(f: FoodDetail): Promise<void> {
    // Densité saisie en g/mL : 1,03 g/mL = 1 030 mg/mL ; poids d'une pièce en g.
    const density = this.density().trim() ? parseNutrient(this.density()) : null;
    const piece = this.piece().trim() ? parseQuantity(this.piece(), 'MASS') : null;
    if ((this.density().trim() && !density) || (this.piece().trim() && !piece)) {
      this.snack.open('Densité ou poids invalide.', 'Fermer');
      return;
    }
    await this.run(
      this.api.updateFood({
        id: f.id,
        body: {
          name: this.name(),
          group_id: this.group(),
          aliases: this.aliases()
            .split(',')
            .map((a) => a.trim())
            .filter(Boolean),
          nutrient_basis: this.basis(),
          density_mg_per_ml: density,
          piece_weight_mg: piece,
          settings: this.settings(),
          minimum_stock: f.minimum_stock ?? null,
          preferred_product_id: f.preferred_product_id ?? null,
          hidden: this.hidden(),
        },
      }),
      'Aliment enregistré',
    );
  }

  protected async saveNutrients(f: FoodDetail): Promise<void> {
    const values = NUTRIENTS.map((n) => {
      const text = (this.values()[n.code] ?? '').trim();
      return {
        code: n.code,
        value: text ? parseNutrient(text) : null,
        invalid: !!text && parseNutrient(text) === null,
      };
    });
    if (values.some((v) => v.invalid)) {
      this.snack.open('Une valeur est invalide.', 'Fermer');
      return;
    }
    // Ne renvoyer que ce qui change, pour ne pas transformer une valeur Ciqual en saisie (R-ALI-11).
    const changed = values.filter((v) => {
      const current = f.nutrients.find((n) => n.code === v.code);
      return (current?.value ?? null) !== v.value;
    });
    if (this.basis() !== f.nutrient_basis) {
      await this.saveGeneral(f);
    }
    if (changed.length) {
      await this.run(
        this.api.setFoodNutrients({
          id: f.id,
          body: { values: changed.map(({ code, value }) => ({ code, value })) },
        }),
        'Valeurs enregistrées',
      );
    }
  }

  protected async restore(f: FoodDetail, code: string): Promise<void> {
    await this.run(this.api.restoreCiqualNutrient({ id: f.id, code }), 'Valeur Ciqual rétablie');
  }

  protected async addUnit(f: FoodDetail): Promise<void> {
    const amount = parseQuantity(this.unitAmount(), this.unitMeasure());
    if (!amount) {
      this.snack.open(
        `Valeur invalide (${UNITS[this.unitMeasure()].map((u) => u.symbol).join(', ')}).`,
        'Fermer',
      );
      return;
    }
    await this.run(
      this.api.addFoodUnit({
        id: f.id,
        body: { name: this.unitName(), measure: this.unitMeasure(), amount },
      }),
      'Unité ajoutée',
    );
    this.unitName.set('');
    this.unitAmount.set('');
  }

  protected async deleteUnit(f: FoodDetail, unit: string): Promise<void> {
    await this.run(this.api.deleteFoodUnit({ id: f.id, unit }), 'Unité supprimée');
  }

  protected async prefer(f: FoodDetail, p: Product): Promise<void> {
    await this.run(
      this.api.updateFood({
        id: f.id,
        body: {
          name: f.name,
          group_id: f.group_id,
          aliases: f.aliases,
          nutrient_basis: f.nutrient_basis,
          density_mg_per_ml: f.density_mg_per_ml ?? null,
          piece_weight_mg: f.piece_weight_mg ?? null,
          settings: f.settings,
          minimum_stock: f.minimum_stock ?? null,
          preferred_product_id: f.preferred_product_id === p.id ? null : p.id,
          hidden: f.hidden,
        },
      }),
      'Produit habituel enregistré',
    );
  }

  protected async editProduct(f: FoodDetail, product?: Product): Promise<void> {
    const ref = this.dialog.open(ProductDialog, {
      data: { foodId: f.id, measure: f.effective.stock_measure, product },
      width: '30rem',
    });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async changeTracking(f: FoodDetail): Promise<void> {
    const ref = this.dialog.open(TrackingDialog, { data: f, width: '30rem' });
    if (await firstValueFrom(ref.afterClosed())) {
      this.version.bump();
    }
  }

  protected async remove(f: FoodDetail): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer ${f.name} ?`,
      message: 'Un aliment qui a des produits ou des lots ne peut pas être supprimé.',
      action: 'Supprimer',
      destructive: true,
    });
    if (!ok) return;
    try {
      await firstValueFrom(this.api.deleteFood({ id: f.id }));
      this.version.bump();
      await this.router.navigate(['/aliments']);
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }

  private async run(request: Observable<unknown>, message: string): Promise<void> {
    this.busy.set(true);
    try {
      await firstValueFrom(request);
      this.version.bump();
      this.snack.open(message, undefined, { duration: 3000 });
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }
}
