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
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';

import { AiProvider } from '../../api/models/ai-provider';
import { AiService } from '../../api/services/ai.service';
import { errorMessage } from '../../core/http-errors';

const DEFAULT_URLS: Record<AiProvider, string> = {
  MISTRAL: 'https://api.mistral.ai/v1',
  OPENAI: 'https://api.openai.com/v1',
};

/**
 * Réglages de l'IA (ADR-0014) : fournisseur, adresse, modèle, clé d'accès (jamais réaffichée), plafonds.
 * Désactivée par défaut (ADR-0007).
 */
@Component({
  selector: 'app-ai-settings',
  imports: [
    FormsModule,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    MatSlideToggleModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card">
      <h2 class="widget-title">Intelligence artificielle</h2>
      <p class="muted">
        Pour créer des recettes et proposer des menus. Rien n'est envoyé sans votre accord : les
        données sont affichées avant chaque envoi, et les membres du foyer y sont désignés par une
        lettre.
      </p>
      @if (current.hasValue()) {
        <form (ngSubmit)="save()">
          <mat-slide-toggle
            name="enabled"
            [ngModel]="enabled()"
            (ngModelChange)="enabled.set($event)"
            >Activer l'IA</mat-slide-toggle
          >
          <div class="form-grid fields">
            <mat-form-field>
              <mat-label>Fournisseur</mat-label>
              <mat-select
                name="provider"
                [ngModel]="provider()"
                (ngModelChange)="setProvider($event)"
              >
                <mat-option value="MISTRAL">Mistral</mat-option>
                <mat-option value="OPENAI">OpenAI ou compatible</mat-option>
              </mat-select>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Modèle</mat-label>
              <input
                matInput
                name="model"
                [ngModel]="model()"
                (ngModelChange)="model.set($event)"
                [placeholder]="provider() === 'MISTRAL' ? 'mistral-small-latest' : 'gpt-4.1-mini'"
              />
            </mat-form-field>
            <mat-form-field class="wide">
              <mat-label>Adresse de l'API</mat-label>
              <input
                matInput
                name="url"
                type="url"
                [ngModel]="baseUrl()"
                (ngModelChange)="baseUrl.set($event)"
                [placeholder]="defaultUrl()"
              />
              <mat-hint
                >Laisser vide pour l'adresse du fournisseur ; à remplacer pour un service compatible
                (LiteLLM…).</mat-hint
              >
            </mat-form-field>
            <mat-form-field class="wide">
              <mat-label>Clé d'accès</mat-label>
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
            <mat-form-field>
              <mat-label>Appels par mois au plus</mat-label>
              <input
                matInput
                type="number"
                min="0"
                name="limit"
                [ngModel]="limit()"
                (ngModelChange)="limit.set($event ?? 0)"
              />
              <mat-hint>{{ used() }} ce mois-ci</mat-hint>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Recettes nouvelles par planning</mat-label>
              <input
                matInput
                type="number"
                min="0"
                name="newRecipes"
                [ngModel]="newRecipes()"
                (ngModelChange)="newRecipes.set($event ?? 0)"
              />
            </mat-form-field>
          </div>
          <div class="actions">
            <button mat-flat-button type="submit" [disabled]="busy()">Enregistrer</button>
            @if (hasKey()) {
              <button mat-button type="button" (click)="clearKey()" [disabled]="busy()">
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
export class AiSettings {
  private readonly api = inject(AiService);
  private readonly snack = inject(MatSnackBar);

  protected readonly current = rxResource({ stream: () => this.api.getAiSettings() });

  // Valeurs du formulaire, initialisées avec les réglages dès leur arrivée (pas après un rendu).
  protected readonly enabled = linkedSignal(() => this.current.value()?.enabled ?? false);
  protected readonly provider = linkedSignal<AiProvider>(
    () => this.current.value()?.provider ?? 'MISTRAL',
  );
  protected readonly model = linkedSignal(() => this.current.value()?.model ?? '');
  protected readonly baseUrl = linkedSignal(() => {
    const s = this.current.value();
    return !s || s.base_url === DEFAULT_URLS[s.provider] ? '' : s.base_url;
  });
  protected readonly hasKey = computed(() => this.current.value()?.has_key ?? false);
  protected readonly limit = linkedSignal(() => this.current.value()?.monthly_limit ?? 50);
  protected readonly newRecipes = linkedSignal(
    () => this.current.value()?.new_recipes_per_plan ?? 3,
  );
  protected readonly used = computed(() => this.current.value()?.used_this_month ?? 0);
  protected readonly key = signal('');
  protected readonly busy = signal(false);

  protected defaultUrl(): string {
    return DEFAULT_URLS[this.provider()];
  }

  protected setProvider(provider: AiProvider): void {
    this.provider.set(provider);
  }

  protected async save(apiKey: string | null = this.key() || null): Promise<void> {
    this.busy.set(true);
    try {
      await firstValueFrom(
        this.api.updateAiSettings({
          body: {
            enabled: this.enabled(),
            provider: this.provider(),
            base_url: this.baseUrl() || null,
            model: this.model(),
            api_key: apiKey,
            monthly_limit: Number(this.limit()) || 0,
            new_recipes_per_plan: Number(this.newRecipes()) || 0,
          },
        }),
      );
      this.key.set('');
      this.snack.open('Réglages de l’IA enregistrés', undefined, { duration: 2500 });
      this.current.reload();
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    } finally {
      this.busy.set(false);
    }
  }

  protected async clearKey(): Promise<void> {
    await this.save('');
  }
}
