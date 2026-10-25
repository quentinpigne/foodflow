import { ChangeDetectionStrategy, Component, inject, input, output, signal } from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { debounceTime, forkJoin, map, of } from 'rxjs';

import { FoodSummary } from '../api/models/food-summary';
import { Product } from '../api/models/product';
import { FoodsService } from '../api/services/foods.service';

/** Ce qu'on peut ranger en stock : un aliment ou un produit (R-STK-1). */
export type StockItem = { kind: 'food'; food: FoodSummary } | { kind: 'product'; product: Product };

export function itemName(item: StockItem): string {
  return item.kind === 'food'
    ? item.food.name
    : [item.product.name, item.product.brand].filter(Boolean).join(' — ');
}

const BARCODE = /^\s*\d{8}(\d{4,5})?\s*$/;

/** Recherche d'un aliment ou d'un produit, par nom, alias ou code-barres (ALI-06). */
@Component({
  selector: 'app-item-picker',
  imports: [FormsModule, MatAutocompleteModule, MatFormFieldModule, MatInputModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <mat-form-field class="full-width">
      <mat-label>{{ label() }}</mat-label>
      <mat-icon matPrefix>search</mat-icon>
      <input
        matInput
        name="item"
        autocomplete="off"
        [ngModel]="text()"
        (ngModelChange)="text.set($event)"
        [matAutocomplete]="auto"
        placeholder="Nom, alias ou code-barres"
      />
      <mat-autocomplete
        #auto="matAutocomplete"
        [displayWith]="display"
        (optionSelected)="pick($event.option.value)"
      >
        @let found = results.value();
        @if (found) {
          @if (found.products.length) {
            <mat-optgroup label="Produits">
              @for (p of found.products; track p.id) {
                <mat-option [value]="{ kind: 'product', product: p }">
                  {{ p.name }}
                  @if (p.brand) {
                    <span class="muted"> — {{ p.brand }}</span>
                  }
                </mat-option>
              }
            </mat-optgroup>
          }
          @if (found.foods.length) {
            <mat-optgroup label="Aliments">
              @for (f of found.foods; track f.id) {
                <mat-option [value]="{ kind: 'food', food: f }">
                  {{ f.name }} <span class="muted"> — {{ f.group_path }}</span>
                </mat-option>
              }
            </mat-optgroup>
          }
        }
      </mat-autocomplete>
    </mat-form-field>
  `,
})
export class ItemPicker {
  private readonly api = inject(FoodsService);

  readonly label = input('Aliment ou produit');
  /** N'affiche que les aliments (pas les produits). */
  readonly foodsOnly = input(false);
  readonly picked = output<StockItem>();

  protected readonly text = signal('');
  private readonly query = toSignal(toObservable(this.text).pipe(debounceTime(200)), {
    initialValue: '',
  });

  protected readonly results = rxResource({
    params: () => (typeof this.query() === 'string' ? this.query().trim() : ''),
    stream: ({ params: q }) => {
      if (q.length < 2) {
        return of({ foods: [] as FoodSummary[], products: [] as Product[] });
      }
      if (BARCODE.test(q)) {
        return this.api
          .searchProducts({ barcode: q.trim() })
          .pipe(map((products) => ({ foods: [] as FoodSummary[], products })));
      }
      return forkJoin({
        foods: this.api.searchFoods({ q }),
        products: this.foodsOnly() ? of([] as Product[]) : this.api.searchProducts({ q }),
      }).pipe(map(({ foods, products }) => ({ foods, products: products.slice(0, 10) })));
    },
  });

  protected readonly display = (value: StockItem | string | null): string =>
    !value ? '' : typeof value === 'string' ? value : itemName(value);

  protected pick(item: StockItem): void {
    this.picked.emit(item);
  }
}
