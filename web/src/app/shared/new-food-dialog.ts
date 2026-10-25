import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialogModule, MatDialogRef } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { firstValueFrom } from 'rxjs';

import { FoodsService } from '../api/services/foods.service';
import { errorMessage } from '../core/http-errors';
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

  protected async save(): Promise<void> {
    const group = this.group();
    if (!group) return;
    this.busy.set(true);
    this.error.set(null);
    try {
      const name = this.name().trim();
      const created = await firstValueFrom(
        this.api.createFood({ body: { name, group_id: group } }),
      );
      this.ref.close({ id: created.id, name } satisfies NewFood);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
