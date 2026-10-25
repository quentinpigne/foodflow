import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { Observable, firstValueFrom } from 'rxjs';

import { FoodGroup } from '../../api/models/food-group';
import { Settings } from '../../api/models/settings';
import { FoodsService } from '../../api/services/foods.service';
import { errorMessage } from '../../core/http-errors';
import { confirm } from '../../shared/confirm-dialog';
import { SettingsEditor } from '../../shared/settings-editor';
import { GroupsStore } from '../../shared/stores';

export interface GroupDialogData {
  group?: FoodGroup;
  parentId?: string | null;
}

/** Famille d'aliments (ALI-01) : nom, place dans l'arbre, réglages hérités ; suppression et fusion (R-ALI-3). */
@Component({
  selector: 'app-group-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatInputModule,
    MatSelectModule,
    SettingsEditor,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>{{ group ? 'Famille' : 'Nouvelle famille' }}</h2>
    <mat-dialog-content>
      <form id="group" class="form" (ngSubmit)="save()">
        <mat-form-field>
          <mat-label>Nom</mat-label>
          <input
            matInput
            name="name"
            [ngModel]="name()"
            (ngModelChange)="name.set($event)"
            required
            maxlength="120"
          />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Famille parente</mat-label>
          <mat-select name="parent" [ngModel]="parent()" (ngModelChange)="parent.set($event)">
            <mat-option [value]="null">Aucune (racine)</mat-option>
            @for (g of parents(); track g.id) {
              <mat-option [value]="g.id">{{ g.path }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <app-settings-editor [(settings)]="settings" [inherited]="inherited()" />
        @if (group) {
          <mat-form-field>
            <mat-label>Fusionner dans</mat-label>
            <mat-select
              name="merge"
              [ngModel]="mergeInto()"
              (ngModelChange)="mergeInto.set($event)"
            >
              <mat-option [value]="null">—</mat-option>
              @for (g of parents(); track g.id) {
                <mat-option [value]="g.id">{{ g.path }}</mat-option>
              }
            </mat-select>
            <mat-hint
              >Tout ce que contient cette famille y est déplacé, puis elle est supprimée.</mat-hint
            >
          </mat-form-field>
        }
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      @if (group) {
        <button mat-button class="danger" (click)="remove()">Supprimer</button>
        @if (mergeInto()) {
          <button mat-button (click)="merge()">Fusionner</button>
        }
      }
      <span class="spacer"></span>
      <button mat-button mat-dialog-close>Annuler</button>
      <button mat-flat-button form="group" type="submit" [disabled]="busy() || !name().trim()">
        Enregistrer
      </button>
    </mat-dialog-actions>
  `,
  styles: `
    .form {
      display: flex;
      flex-direction: column;
      gap: var(--field-gap);
    }
    .error,
    .danger {
      color: var(--mat-sys-error);
    }
    .spacer {
      flex: 1;
    }
  `,
})
export class GroupDialog {
  private readonly data = inject<GroupDialogData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<GroupDialog>);
  private readonly api = inject(FoodsService);
  private readonly dialog = inject(MatDialog);
  private readonly groups = inject(GroupsStore);

  protected readonly group = this.data.group;
  protected readonly name = signal(this.group?.name ?? '');
  protected readonly parent = signal<string | null>(
    this.group?.parent_id ?? this.data.parentId ?? null,
  );
  protected readonly settings = signal<Settings>({ ...(this.group?.settings ?? {}) });
  protected readonly mergeInto = signal<string | null>(null);
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);

  /** Parents possibles : toutes les familles, sauf elle-même et ses descendantes (R-ALI-1). */
  protected readonly parents = computed(() => {
    const id = this.group?.id;
    if (!id) {
      return this.groups.list();
    }
    const own = this.groups.list().find((g) => g.id === id)?.path ?? '';
    return this.groups.list().filter((g) => g.id !== id && !g.path.startsWith(own + ' › '));
  });
  protected readonly inherited = computed(
    () => this.groups.list().find((g) => g.id === this.parent())?.effective ?? null,
  );

  protected async save(): Promise<void> {
    const body = {
      name: this.name(),
      parent_id: this.parent(),
      settings: this.settings(),
      reference_food_id: this.group?.reference_food_id ?? null,
    };
    await this.run(
      this.group
        ? this.api.updateFoodGroup({ id: this.group.id, body })
        : this.api.createFoodGroup({ body }),
    );
  }

  protected async remove(): Promise<void> {
    if (!this.group) return;
    const ok = await confirm(this.dialog, {
      title: 'Supprimer la famille ?',
      message: 'Seule une famille vide peut être supprimée ; sinon, la fusionner dans une autre.',
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) await this.run(this.api.deleteFoodGroup({ id: this.group.id }));
  }

  protected async merge(): Promise<void> {
    const into = this.mergeInto();
    if (this.group && into)
      await this.run(this.api.mergeFoodGroup({ id: this.group.id, body: { into } }));
  }

  private async run(request: Observable<unknown>): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      await firstValueFrom(request);
      this.ref.close(true);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
