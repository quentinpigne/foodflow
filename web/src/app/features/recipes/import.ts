import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { DomSanitizer } from '@angular/platform-browser';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { firstValueFrom } from 'rxjs';

import { RecipesService } from '../../api/services/recipes.service';
import { errorMessage } from '../../core/http-errors';
import { Layout } from '../../core/layout';
import { FrDatePipe } from '../../shared/date.pipe';
import { SharedImport, bookmarklet, extractUrls, readShared } from './import-links';

/** Pause entre deux pages d'un lot : on ne martèle pas le site (R-REC-69). */
const PAUSE_MS = 2_000;

interface QueueItem {
  url: string;
  state: 'waiting' | 'running' | 'done' | 'skipped' | 'error';
  message?: string;
  draftId?: string;
}

/**
 * Import de recettes (REC-07, REC-11, REC-17, REC-18) : une ou plusieurs adresses, un fichier JSON-LD
 * exporté, ou ce qu'envoient le favori et le partage ; brouillons en attente.
 */
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
        <h1>Importer des recettes</h1>
      </div>
      <section class="card">
        <p class="muted">
          La page doit publier sa recette au format schema.org (c'est le cas de la plupart des sites
          de recettes). Chaque adresse donne un brouillon : rien n'est enregistré avant sa
          validation.
        </p>
        <form (ngSubmit)="importUrls()">
          <mat-form-field class="full-width">
            <mat-label>Adresses des recettes</mat-label>
            <textarea
              matInput
              name="urls"
              rows="4"
              [ngModel]="text()"
              (ngModelChange)="text.set($event)"
              placeholder="https://… (une par ligne)"
            ></textarea>
            <mat-hint>{{ urlCount() }} adresse(s) · importées une par une</mat-hint>
          </mat-form-field>
          <div class="row">
            <button mat-flat-button type="submit" [disabled]="busy() || !urlCount()">
              Importer
            </button>
            <button mat-stroked-button type="button" [disabled]="busy()" (click)="file.click()">
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
        </form>
        @if (busy() && !queue().length) {
          <mat-progress-bar mode="indeterminate" />
        }
        @if (error()) {
          <p class="error">{{ error() }}</p>
        }
      </section>

      @if (queue().length) {
        <section class="card">
          <div class="section-head">
            <h2 class="widget-title">Lot en cours · {{ finished() }} / {{ queue().length }}</h2>
            @if (busy()) {
              <button mat-button type="button" (click)="stop()">Arrêter</button>
            }
          </div>
          <mat-progress-bar mode="determinate" [value]="(finished() / queue().length) * 100" />
          @for (item of queue(); track item.url) {
            <div class="queue-item" [class]="item.state">
              <mat-icon>{{ icons[item.state] }}</mat-icon>
              <div>
                <div class="url">{{ item.url }}</div>
                @if (item.message) {
                  <div class="meta">{{ item.message }}</div>
                }
              </div>
            </div>
          }
        </section>
      }

      @if (drafts.value()?.length) {
        <section class="card">
          <div class="section-head">
            <h2 class="widget-title">Brouillons en attente ({{ drafts.value()?.length }})</h2>
            <a mat-flat-button [routerLink]="['/recettes/import', oldest()]">Relire</a>
          </div>
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

      <section class="card">
        <h2 class="widget-title">Envoyer depuis le navigateur</h2>
        @if (layout.mobile()) {
          <p>
            Installez Foodflow sur l'écran d'accueil (menu du navigateur, « Installer l'application
            »). Il apparaît alors dans le menu <strong>Partager</strong> : partagez une recette
            depuis n'importe quel site ou application pour en faire un brouillon.
          </p>
          <p class="muted">
            Le partage demande Chrome ou Edge sur Android, et Foodflow servi en HTTPS.
          </p>
        } @else {
          <p>
            Glissez ce bouton dans la barre de favoris. Sur la page d'une recette, cliquez dessus :
            la recette s'ouvre ici en brouillon, sans que Foodflow retélécharge la page.
          </p>
          <p>
            <a
              class="bookmarklet"
              mat-stroked-button
              [href]="bookmarkletHref"
              (click)="$event.preventDefault()"
              ><mat-icon>bookmark_add</mat-icon>Envoyer à Foodflow</a
            >
          </p>
        }
      </section>
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
      margin: 0.5rem 0;
    }
    .full-width {
      width: 100%;
    }
    .section-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
    }
    .queue-item {
      display: flex;
      align-items: flex-start;
      gap: 0.75rem;
      padding: 0.5rem 0;
    }
    .queue-item .url {
      word-break: break-all;
    }
    .queue-item.done mat-icon {
      color: var(--mat-sys-primary);
    }
    .queue-item.error mat-icon {
      color: var(--mat-sys-error);
    }
    .queue-item.waiting,
    .queue-item.skipped {
      color: var(--mat-sys-on-surface-variant);
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .bookmarklet {
      cursor: grab;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class RecipeImportPage {
  private readonly api = inject(RecipesService);
  private readonly router = inject(Router);
  protected readonly layout = inject(Layout);

  protected readonly text = signal('');
  protected readonly busy = signal(false);
  protected readonly error = signal<string | null>(null);
  protected readonly queue = signal<QueueItem[]>([]);
  protected readonly drafts = rxResource({ stream: () => this.api.listRecipeDrafts() });

  protected readonly urlCount = computed(() => extractUrls(this.text()).length);
  protected readonly finished = computed(
    () => this.queue().filter((i) => i.state !== 'waiting' && i.state !== 'running').length,
  );
  /** Les brouillons se relisent dans l'ordre de leur import. */
  protected readonly oldest = computed(() => this.drafts.value()?.at(-1)?.id);

  protected readonly icons: Record<QueueItem['state'], string> = {
    waiting: 'schedule',
    running: 'downloading',
    done: 'check_circle',
    skipped: 'block',
    error: 'error',
  };

  // Un lien « javascript: » est neutralisé par Angular s'il n'est pas déclaré sûr : c'est le favori voulu.
  protected readonly bookmarkletHref = inject(DomSanitizer).bypassSecurityTrustUrl(
    bookmarklet(location.origin),
  );

  private stopped = false;

  constructor() {
    inject(DestroyRef).onDestroy(() => (this.stopped = true));
    const route = inject(ActivatedRoute).snapshot;
    const shared = readShared(route.fragment, new URLSearchParams(route.queryParams));
    if (shared) {
      void this.start(shared, true);
    }
  }

  protected async fromFile(event: Event): Promise<void> {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) {
      await this.start({ json_ld: await file.text() });
    }
  }

  protected async importUrls(): Promise<void> {
    const urls = extractUrls(this.text());
    if (urls.length === 1) {
      await this.start({ url: urls[0] });
      return;
    }
    // Une adresse qui a déjà un brouillon en attente n'est pas retéléchargée (R-REC-69).
    const pending = new Set((this.drafts.value() ?? []).map((d) => d.source_url));
    this.queue.set(
      urls.map((url) =>
        pending.has(url)
          ? { url, state: 'skipped', message: 'Déjà un brouillon en attente.' }
          : { url, state: 'waiting' },
      ),
    );
    this.busy.set(true);
    this.error.set(null);
    this.stopped = false;
    let first = true;
    for (const item of this.queue()) {
      if (this.stopped) break;
      if (item.state !== 'waiting') continue;
      if (!first) await new Promise((resolve) => setTimeout(resolve, PAUSE_MS));
      first = false;
      if (this.stopped) break;
      this.update(item.url, { state: 'running' });
      try {
        const draft = await firstValueFrom(this.api.startRecipeImport({ body: { url: item.url } }));
        this.update(item.url, { state: 'done', message: draft.payload.name, draftId: draft.id });
      } catch (error) {
        this.update(item.url, { state: 'error', message: errorMessage(error) });
      }
    }
    this.queue.update((items) =>
      items.map((i) =>
        i.state === 'waiting' ? { ...i, state: 'skipped', message: 'Arrêté.' } : i,
      ),
    );
    this.busy.set(false);
    this.text.set(
      this.queue()
        .filter((i) => i.state === 'error')
        .map((i) => i.url)
        .join('\n'),
    );
    this.drafts.reload();
  }

  protected stop(): void {
    this.stopped = true;
  }

  private update(url: string, change: Partial<QueueItem>): void {
    this.queue.update((items) => items.map((i) => (i.url === url ? { ...i, ...change } : i)));
  }

  /** Une recette seule : on ouvre directement sa revue. */
  private async start(body: SharedImport, replaceUrl = false): Promise<void> {
    this.busy.set(true);
    this.error.set(null);
    try {
      const draft = await firstValueFrom(this.api.startRecipeImport({ body }));
      await this.router.navigate(['/recettes/import', draft.id], { replaceUrl });
    } catch (error) {
      this.error.set(errorMessage(error));
      if (replaceUrl) {
        // Le favori ou le partage a échoué : on garde le message, sans les données dans l'adresse.
        await this.router.navigate(['/recettes/import'], { replaceUrl: true });
      }
    } finally {
      this.busy.set(false);
    }
  }
}
