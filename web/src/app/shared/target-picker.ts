import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { rxResource, toObservable, toSignal } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { debounceTime, firstValueFrom, map, of } from 'rxjs';

import { Target } from '../api/models/target';
import { FoodsService } from '../api/services/foods.service';
import { RecipesService } from '../api/services/recipes.service';
import { DataVersion } from '../core/data-version';
import { fold } from '../core/text';
import { NewFood, NewFoodDialog } from './new-food-dialog';
import { GroupsStore } from './stores';

export interface PickedTarget {
  target: Target;
  name: string;
}

/** Cible d'un ingrédient (R-REC-20) : un aliment, une famille (« au choix ») ou une sous-recette. */
@Component({
  selector: 'app-target-picker',
  imports: [FormsModule, MatAutocompleteModule, MatFormFieldModule, MatInputModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <mat-form-field class="full-width" [class.missing]="!name()">
      <mat-label>{{ label() }}</mat-label>
      <input
        matInput
        autocomplete="off"
        [ngModel]="text()"
        (ngModelChange)="text.set($event)"
        [matAutocomplete]="auto"
        placeholder="Aliment, famille ou recette"
      />
      <mat-autocomplete
        #auto="matAutocomplete"
        [displayWith]="display"
        (optionSelected)="pick($event.option.value)"
      >
        @if (foodResults.value()?.length) {
          <mat-optgroup label="Aliments">
            @for (f of foodResults.value(); track f.target.id) {
              <mat-option [value]="f">{{ f.name }}</mat-option>
            }
          </mat-optgroup>
        }
        @if (groups().length) {
          <mat-optgroup label="Familles (au choix)">
            @for (g of groups(); track g.target.id) {
              <mat-option [value]="g">{{ g.name }}</mat-option>
            }
          </mat-optgroup>
        }
        @if (recipeResults.value()?.length) {
          <mat-optgroup label="Recettes">
            @for (rec of recipeResults.value(); track rec.target.id) {
              <mat-option [value]="rec">{{ rec.name }}</mat-option>
            }
          </mat-optgroup>
        }
        @if (searching()) {
          <mat-option disabled>Recherche…</mat-option>
        } @else if (canCreate()) {
          <mat-option [value]="create">Créer l'aliment « {{ q() }} »</mat-option>
        }
      </mat-autocomplete>
    </mat-form-field>
  `,
  styles: `
    .missing {
      --mat-form-field-outlined-outline-color: var(--mat-sys-error);
    }
  `,
})
export class TargetPicker {
  private readonly foods = inject(FoodsService);
  private readonly recipes = inject(RecipesService);
  private readonly groupStore = inject(GroupsStore);
  private readonly dialog = inject(MatDialog);
  private readonly version = inject(DataVersion);

  /** Option « Créer l'aliment » : un ingrédient sans aliment existant (R-REC-66). */
  protected readonly create = { create: true } as const;

  readonly label = input('Ingrédient');
  /** Nom de la cible déjà choisie. */
  readonly name = input<string | null | undefined>(null);
  /** Recette en cours d'édition : elle ne peut pas être son propre ingrédient (R-REC-30). */
  readonly exclude = input<string | null>(null);
  readonly picked = output<PickedTarget>();

  protected readonly text = signal<string | PickedTarget>('');
  private readonly query = toSignal(toObservable(this.text).pipe(debounceTime(200)), {
    initialValue: '',
  });
  protected readonly q = computed(() => {
    const value = this.query();
    return typeof value === 'string' ? value.trim() : '';
  });

  constructor() {
    effect(() => this.text.set(this.name() ?? ''));
  }

  // Deux recherches séparées : les aliments s'affichent sans attendre les recettes.
  protected readonly foodResults = rxResource({
    params: () => this.q(),
    stream: ({ params: q }) =>
      q.length < 2
        ? of([] as PickedTarget[])
        : this.foods
            .searchFoods({ q })
            .pipe(
              map((foods) =>
                foods
                  .slice(0, 12)
                  .map((f) => ({ target: { kind: 'FOOD' as const, id: f.id }, name: f.name })),
              ),
            ),
  });

  protected readonly recipeResults = rxResource({
    params: () => this.q(),
    stream: ({ params: q }) =>
      q.length < 2
        ? of([] as PickedTarget[])
        : this.recipes.searchRecipes({ q }).pipe(
            map((recipes) =>
              recipes
                .filter((r) => r.id !== this.exclude())
                .slice(0, 6)
                .map((r) => ({ target: { kind: 'RECIPE' as const, id: r.id }, name: r.name })),
            ),
          ),
  });

  /** Saisie en attente de la recherche (anti-rebond) ou recherche d'aliments en cours. */
  protected readonly searching = computed(() => {
    const text = this.text();
    const typed = typeof text === 'string' ? text.trim() : '';
    return typed.length >= 2 && (typed !== this.q() || this.foodResults.isLoading());
  });

  /**
   * « Créer l'aliment » : une fois les aliments trouvés, et seulement si aucun ne porte déjà ce nom — un aliment
   * masqué de la table Ciqual (« Rhum ») est trouvé et proposé, pas recréé.
   */
  protected readonly canCreate = computed(() => {
    const q = fold(this.q());
    if (q.length < 2 || !this.foodResults.hasValue()) {
      return false;
    }
    return !this.foodResults.value().some((f) => fold(f.name) === q);
  });

  protected readonly groups = computed<PickedTarget[]>(() => {
    const words = fold(this.q()).split(/\s+/).filter(Boolean);
    if (!words.length || this.q().length < 2) {
      return [];
    }
    return this.groupStore
      .list()
      .filter((g) => words.every((w) => fold(g.name).includes(w)))
      .slice(0, 6)
      .map((g) => ({ target: { kind: 'GROUP' as const, id: g.id }, name: g.name }));
  });

  protected readonly display = (value: PickedTarget | string | null): string =>
    !value ? '' : typeof value === 'string' ? value : 'name' in value ? value.name : '';

  protected async pick(value: PickedTarget | typeof this.create): Promise<void> {
    if (!('create' in value)) {
      this.picked.emit(value);
      return;
    }
    const ref = this.dialog.open(NewFoodDialog, {
      data: { groupId: null, name: this.q() },
      width: '30rem',
    });
    const created: NewFood | undefined = await firstValueFrom(ref.afterClosed());
    if (created) {
      this.version.bump();
      this.picked.emit({ target: { kind: 'FOOD', id: created.id }, name: created.name });
    }
    this.text.set(created?.name ?? this.name() ?? '');
  }
}
