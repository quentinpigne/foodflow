import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { RecipesService } from '../../api/services/recipes.service';
import { errorMessage } from '../../core/http-errors';
import { FrDatePipe } from '../../shared/date.pipe';

/** Import d'une recette (REC-07, REC-11) : une adresse, ou un fichier JSON-LD exporté ; brouillons en attente. */
@Component({
  selector: 'app-recipe-import',
  imports: [
    FormsModule,
    RouterLink,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <a mat-icon-button routerLink="/recettes" aria-label="Retour aux recettes"
          ><mat-icon>arrow_back</mat-icon></a
        >
        <h1>Importer une recette</h1>
      </div>
      <section class="card">
        <p class="muted">
          La page doit publier sa recette au format schema.org (c'est le cas de la plupart des sites
          de recettes). L'import crée un brouillon : rien n'est enregistré avant sa validation.
        </p>
        <form class="row" (ngSubmit)="start({ url: url() })">
          <mat-form-field class="grow">
            <mat-label>Adresse de la recette</mat-label>
            <input
              matInput
              name="url"
              type="url"
              [ngModel]="url()"
              (ngModelChange)="url.set($event)"
              placeholder="https://…"
            />
          </mat-form-field>
          <button mat-flat-button type="submit" [disabled]="busy() || !url().trim()">
            Importer
          </button>
        </form>
        <div class="row">
          <button mat-stroked-button type="button" (click)="file.click()">
            <mat-icon>upload_file</mat-icon>Fichier JSON-LD exporté
          </button>
          <input
            #file
            type="file"
            accept=".jsonld,.json,application/ld+json,application/json"
            hidden
            (change)="fromFile($event)"
          />
        </div>
        @if (busy()) {
          <mat-progress-bar mode="indeterminate" />
        }
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </section>
      @if (drafts.value()?.length) {
        <section class="card">
          <h2 class="widget-title">Brouillons en attente</h2>
          @for (d of drafts.value(); track d.id) {
            <a class="lot-row" [routerLink]="['/recettes/import', d.id]">
              <div>
                <div class="name">{{ d.payload.name }}</div>
                <div class="meta">
                  {{ d.source_url ?? 'fichier' }} · importé le
                  {{ d.fetched_at.slice(0, 10) | frDate }}
                </div>
              </div>
            </a>
          }
        </section>
      }
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin-bottom: 0.5rem;
    }
    .grow {
      flex: 1;
      min-width: 16rem;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class RecipeImportPage {
  private readonly api = inject(RecipesService);
  private readonly router = inject(Router);

  protected readonly url = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly drafts = rxResource({ stream: () => this.api.listRecipeDrafts() });

  protected async fromFile(event: Event): Promise<void> {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) {
      await this.start({ json_ld: await file.text() });
    }
  }

  protected async start(body: { url?: string; json_ld?: string }): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      const draft = await firstValueFrom(this.api.startRecipeImport({ body }));
      await this.router.navigate(['/recettes/import', draft.id]);
    } catch (error) {
      this.error.set(errorMessage(error));
    } finally {
      this.busy.set(false);
    }
  }
}
