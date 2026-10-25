import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom } from 'rxjs';

import { FoodsService } from '../api/services/foods.service';
import { errorMessage, problemOf } from '../core/http-errors';
import { fold } from '../core/text';
import { GroupsStore } from './stores';

export interface NewFoodData {
  groupId: string | null;
  /** Nom proposé : celui d'un ingrédient importé sans aliment (R-REC-66). */
  name?: string;
}

export interface NewFood {
  id: string;
  name: string;
}

/** Nouvel aliment (ALI-02) : nom et famille ; le reste se règle sur sa fiche. */
@Component({
  selector: 'app-new-food-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Nouvel aliment</h2>
    <mat-dialog-content>
      <form id="new-food" class="form" (ngSubmit)="save()">
        <mat-form-field>
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
          <mat-label>Famille</mat-label>
          <mat-select name="group" [ngModel]="group()" (ngModelChange)="group.set($event)" required>
            @for (g of groups.list(); track g.id) {
              <mat-option [value]="g.id">{{ g.path }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
        @if (existing(); as e) {
          <button mat-stroked-button type="button" (click)="useExisting(e)">
            Utiliser l'aliment existant « {{ e.name }} »
          </button>
        }
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button mat-dialog-close>Annuler</button>
      <button
        mat-flat-button
        form="new-food"
        type="submit"
        [disabled]="busy() || !name().trim() || !group()"
      >
        Créer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class NewFoodDialog {
  private readonly data = inject<NewFoodData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<NewFoodDialog>);
  private readonly api = inject(FoodsService);
  protected readonly groups = inject(GroupsStore);

  protected readonly name = signal(this.data.name ?? '');
  protected readonly group = signal<string | null>(this.data.groupId);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  /** Aliment qui porte déjà ce nom (souvent un aliment Ciqual masqué) : on le propose plutôt qu'un doublon. */
  protected readonly existing = signal<NewFood | null>(null);

  protected async save(): Promise<void> {
    const group = this.group();
    if (!group) return;
    this.busy.set(true);
    this.error.set(null);
    this.existing.set(null);
    try {
      const name = this.name().trim();
      const created = await firstValueFrom(
        this.api.createFood({ body: { name, group_id: group } }),
      );
      this.ref.close({ id: created.id, name } satisfies NewFood);
    } catch (error) {
      this.error.set(errorMessage(error));
      if (['FOOD_NAME_TAKEN', 'ALIAS_TAKEN'].includes(problemOf(error)?.code ?? '')) {
        this.existing.set(await this.findExisting(this.name().trim()));
      }
    } finally {
      this.busy.set(false);
    }
  }

  protected useExisting(food: NewFood): void {
    this.ref.close(food);
  }

  private async findExisting(name: string): Promise<NewFood | null> {
    const found = await firstValueFrom(this.api.searchFoods({ q: name, include_hidden: true }));
    const same = found.find((f) => fold(f.name) === fold(name)) ?? found[0];
    return same ? { id: same.id, name: same.name } : null;
  }
}
