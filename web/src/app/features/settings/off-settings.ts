import { ChangeDetectionStrategy, Component, inject, linkedSignal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { firstValueFrom } from 'rxjs';

import { SystemService } from '../../api/services/system.service';
import { errorMessage } from '../../core/http-errors';

/**
 * Open Food Facts (ALI-08) : un code-barres inconnu est cherché dans cette base ouverte pour proposer un
 * brouillon de produit (R-ALI-60). Activé par défaut ; seul le code-barres est envoyé (ADR-0007).
 */
@Component({
  selector: 'app-off-settings',
  imports: [MatSlideToggleModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card">
      <h2 class="widget-title">Open Food Facts</h2>
      <p class="muted">
        Un code-barres inconnu est cherché dans
        <a href="https://fr.openfoodfacts.org/" target="_blank" rel="noopener">Open Food Facts</a>
        pour préremplir le produit : nom, marque, conditionnement, valeurs nutritionnelles. Seul le
        code-barres est envoyé ; le produit n’est créé qu’après validation.
      </p>
      @if (current.hasValue()) {
        <mat-slide-toggle [checked]="enabled()" (change)="save($event.checked)"
          >Chercher les produits inconnus</mat-slide-toggle
        >
      }
    </section>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
  `,
})
export class OffSettings {
  private readonly api = inject(SystemService);
  private readonly snack = inject(MatSnackBar);

  protected readonly current = rxResource({ stream: () => this.api.getOffSettings() });
  protected readonly enabled = linkedSignal(() => this.current.value()?.enabled ?? true);

  protected async save(enabled: boolean): Promise<void> {
    this.enabled.set(enabled);
    try {
      await firstValueFrom(this.api.updateOffSettings({ body: { enabled } }));
      this.snack.open(enabled ? 'Open Food Facts activé' : 'Open Food Facts désactivé', undefined, {
        duration: 2500,
      });
    } catch (error) {
      this.enabled.set(!enabled);
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }
}
