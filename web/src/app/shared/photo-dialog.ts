import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { PhotoCandidate } from '../api/models/photo-candidate';
import { RecipesService } from '../api/services/recipes.service';
import { errorMessage, problemOf } from '../core/http-errors';

/** Photo retenue : une adresse (photo proposée ou saisie) ou un fichier envoyé. */
export type ChosenPhoto = { url: string } | { file: File };

/**
 * Changer de photo (R-REC-73) : photos libres de Pixabay proposées d'après des mots-clés modifiables,
 * image depuis une adresse, ou fichier envoyé. Rien ne change avant le choix.
 */
@Component({
  selector: 'app-photo-dialog',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDialogModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    RouterLink,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Changer de photo</h2>
    <mat-dialog-content>
      <form class="row" (ngSubmit)="search()">
        <mat-form-field class="grow" subscriptSizing="dynamic">
          <mat-label>Chercher une photo</mat-label>
          <input matInput name="query" [ngModel]="query()" (ngModelChange)="query.set($event)" />
        </mat-form-field>
        <button mat-stroked-button type="submit" [disabled]="loading() || !query().trim()">
          <mat-icon>search</mat-icon>Chercher
        </button>
      </form>
      @if (loading()) {
        <mat-progress-bar mode="indeterminate" />
      }
      @if (error(); as message) {
        <p class="muted">
          {{ message }}
          @if (disabled()) {
            <a routerLink="/reglages" mat-dialog-close>Ouvrir les réglages</a>
          }
        </p>
      }
      @if (photos().length) {
        <div class="grid">
          @for (p of photos(); track p.image_url; let i = $index) {
            <button
              type="button"
              class="choice"
              [attr.aria-label]="'Photo ' + (i + 1)"
              (click)="ref.close({ url: p.image_url })"
            >
              <img [src]="p.preview_url" alt="" loading="lazy" />
            </button>
          }
        </div>
        <p class="credit">
          Photos : <a href="https://pixabay.com/" target="_blank" rel="noopener">Pixabay</a>
        </p>
      } @else if (searched() && !loading() && !error()) {
        <p class="muted">Aucune photo trouvée : essayer d'autres mots-clés.</p>
      }
      <form class="row" (ngSubmit)="useUrl()">
        <mat-form-field class="grow" subscriptSizing="dynamic">
          <mat-label>Depuis une adresse</mat-label>
          <input
            matInput
            name="url"
            type="url"
            placeholder="https://…/photo.jpg"
            [ngModel]="url()"
            (ngModelChange)="url.set($event)"
          />
        </mat-form-field>
        <button mat-stroked-button type="submit" [disabled]="!url().trim()">Utiliser</button>
      </form>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button type="button" (click)="fileInput.click()">
        <mat-icon>upload</mat-icon>Envoyer un fichier
      </button>
      <input
        #fileInput
        type="file"
        accept="image/jpeg,image/png,image/webp"
        hidden
        (change)="useFile($event)"
      />
      <button mat-button mat-dialog-close>Annuler</button>
    </mat-dialog-actions>
  `,
  styles: `
    .row {
      display: flex;
      gap: 0.5rem;
      align-items: center;
      margin: 0.5rem 0;
    }
    .grow {
      flex: 1;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 0.5rem;
      margin-top: 0.5rem;
    }
    .choice {
      padding: 0;
      border: 2px solid transparent;
      border-radius: var(--mat-sys-corner-small);
      background: var(--mat-sys-surface-container);
      cursor: pointer;
      overflow: hidden;
      aspect-ratio: 3 / 2;
    }
    .choice:hover,
    .choice:focus-visible {
      border-color: var(--mat-sys-primary);
    }
    .choice img {
      width: 100%;
      height: 100%;
      object-fit: cover;
      display: block;
    }
    .credit {
      font: var(--mat-sys-body-small);
      text-align: right;
      margin: 0.25rem 0 0.75rem;
    }
  `,
})
export class PhotoDialog {
  private readonly api = inject(RecipesService);
  protected readonly ref = inject<MatDialogRef<PhotoDialog, ChosenPhoto>>(MatDialogRef);
  private readonly data = inject<{ query: string }>(MAT_DIALOG_DATA);

  protected readonly query = signal(this.data.query);
  protected readonly url = signal('');
  protected readonly photos = signal<PhotoCandidate[]>([]);
  protected readonly loading = signal(false);
  protected readonly searched = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly disabled = signal(false);

  constructor() {
    if (this.query().trim()) void this.search();
  }

  protected async search(): Promise<void> {
    this.loading.set(true);
    this.error.set(null);
    try {
      this.photos.set(await firstValueFrom(this.api.searchPhotos({ q: this.query().trim() })));
    } catch (error) {
      this.photos.set([]);
      this.error.set(errorMessage(error));
      this.disabled.set(problemOf(error)?.code === 'PHOTOS_DISABLED');
    } finally {
      this.loading.set(false);
      this.searched.set(true);
    }
  }

  protected useUrl(): void {
    this.ref.close({ url: this.url().trim() });
  }

  protected useFile(event: Event): void {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) this.ref.close({ file });
  }
}

/** Ouvre le choix d'une photo, avec des mots-clés de départ ; `undefined` si rien n'est choisi. */
export async function choosePhoto(
  dialog: MatDialog,
  query: string,
): Promise<ChosenPhoto | undefined> {
  const ref = dialog.open<PhotoDialog, { query: string }, ChosenPhoto>(PhotoDialog, {
    data: { query },
    width: '40rem',
    maxWidth: '95vw',
    autoFocus: false,
  });
  return firstValueFrom(ref.afterClosed());
}
