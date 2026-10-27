import {
  ChangeDetectionStrategy,
  Component,
  computed,
  inject,
  linkedSignal,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';

import { SystemService } from '../../api/services/system.service';
import { errorMessage } from '../../core/http-errors';

/**
 * Photos libres des recettes (R-REC-73) : une recette créée sans image reçoit une photo de Pixabay,
 * cherchée d'après son nom. Désactivées par défaut (ADR-0007) ; la clé n'est jamais réaffichée.
 */
@Component({
  selector: 'app-photo-settings',
  imports: [FormsModule, MatButtonModule, MatFormFieldModule, MatInputModule, MatSlideToggleModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card">
      <h2 class="widget-title">Photos des recettes</h2>
      <p class="muted">
        Une recette créée sans image (par l'IA, ou importée d'une page qui n'en a pas) reçoit une
        photo libre de
        <a href="https://pixabay.com/" target="_blank" rel="noopener">Pixabay</a>, cherchée d'après
        son nom. Seul le nom de la recette est envoyé. Clé gratuite sur
        <a href="https://pixabay.com/api/docs/" target="_blank" rel="noopener"
          >pixabay.com/api/docs</a
        >.
      </p>
      @if (current.hasValue()) {
        <form (ngSubmit)="save()">
          <mat-slide-toggle
            name="enabled"
            [ngModel]="enabled()"
            (ngModelChange)="enabled.set($event)"
            >Chercher une photo</mat-slide-toggle
          >
          <div class="form-grid fields">
            <mat-form-field class="wide">
              <mat-label>Clé Pixabay</mat-label>
              <input
                matInput
                name="key"
                type="password"
                autocomplete="off"
                [ngModel]="key()"
                (ngModelChange)="key.set($event)"
                [placeholder]="
                  hasKey() ? 'Clé enregistrée — saisir pour la remplacer' : 'Aucune clé'
                "
              />
              <mat-hint>Jamais réaffichée, ni exportée.</mat-hint>
            </mat-form-field>
          </div>
          <div class="actions">
            <button mat-flat-button type="submit" [disabled]="busy()">Enregistrer</button>
            @if (hasKey()) {
              <button mat-button type="button" (click)="save('')" [disabled]="busy()">
                Effacer la clé
              </button>
            }
          </div>
        </form>
      }
    </section>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .fields {
      margin: 1rem 0;
    }
    .wide {
      grid-column: 1 / -1;
    }
  `,
})
export class PhotoSettings {
  private readonly api = inject(SystemService);
  private readonly snack = inject(MatSnackBar);

  protected readonly current = rxResource({ stream: () => this.api.getPhotoSettings() });
  protected readonly enabled = linkedSignal(() => this.current.value()?.enabled ?? false);
  protected readonly hasKey = computed(() => this.current.value()?.has_key ?? false);
  protected readonly key = signal('');
  protected readonly busy = signal(false);

  protected async save(apiKey: string | null = this.key() || null): Promise<void> {
    this.busy.set(true);
    try {
      await firstValueFrom(
        this.api.updatePhotoSettings({ body: { enabled: this.enabled(), api_key: apiKey } }),
      );
      this.key.set('');
      this.snack.open('Réglages des photos enregistrés', undefined, { duration: 2500 });
      this.current.reload();
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }
}
