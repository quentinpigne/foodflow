import { ChangeDetectionStrategy, Component, effect, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Observable, firstValueFrom } from 'rxjs';

import { Location } from '../../api/models/location';
import { LocationKind } from '../../api/models/location-kind';
import { StockService } from '../../api/services/stock.service';
import { SystemService } from '../../api/services/system.service';
import { Session } from '../../core/auth';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import { LOCATION_KIND_LABELS } from '../../core/labels';
import { confirm } from '../../shared/confirm-dialog';
import { AiSettings } from './ai-settings';
import { OffSettings } from './off-settings';
import { PhotoSettings } from './photo-settings';
import { FrDatePipe } from '../../shared/date.pipe';
import { LocationsStore } from '../../shared/stores';

interface LocationDraft {
  id: string | null;
  name: string;
  kind: LocationKind;
  soon_days: number | null;
}

/** Réglages : emplacements (STK-01), seuil « bientôt » (R-GAS-2), export (ADR-0008), version. */
@Component({
  selector: 'app-settings',
  imports: [
    FormsModule,
    MatButtonModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    FrDatePipe,
    AiSettings,
    OffSettings,
    PhotoSettings,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Réglages</h1>
      </div>

      <section class="card">
        <h2 class="widget-title">Emplacements</h2>
        @for (l of locations.list(); track l.id) {
          <div class="line">
            <span class="grow">
              {{ l.name }}
              <span class="muted"
                >· {{ kindLabels[l.kind] }} · {{ l.lot_count }} lot{{ l.lot_count > 1 ? 's' : '' }}
                @if ((l.soon_days ?? null) !== null) {
                  · bientôt à {{ l.soon_days }} j
                }
              </span>
            </span>
            <button mat-icon-button aria-label="Modifier" (click)="edit(l)">
              <mat-icon>edit</mat-icon>
            </button>
            <button mat-icon-button aria-label="Supprimer" (click)="remove(l)">
              <mat-icon>delete</mat-icon>
            </button>
          </div>
        }
        @if (draft(); as d) {
          <form class="row" (ngSubmit)="saveLocation(d)">
            <mat-form-field>
              <mat-label>Nom</mat-label>
              <input
                matInput
                name="name"
                [ngModel]="d.name"
                (ngModelChange)="patch({ name: $event })"
                required
              />
            </mat-form-field>
            <mat-form-field>
              <mat-label>Nature</mat-label>
              <mat-select name="kind" [ngModel]="d.kind" (ngModelChange)="patch({ kind: $event })">
                @for (k of kinds; track k) {
                  <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-form-field>
              <mat-label>Seuil « bientôt »</mat-label>
              <input
                matInput
                name="soon"
                type="number"
                min="0"
                placeholder="réglage général"
                [ngModel]="d.soon_days"
                (ngModelChange)="patch({ soon_days: $event === '' ? null : $event })"
              />
              <span matTextSuffix>j</span>
            </mat-form-field>
            <button mat-flat-button type="submit">Enregistrer</button>
            <button mat-button type="button" (click)="draft.set(null)">Annuler</button>
          </form>
        } @else {
          <button mat-stroked-button (click)="edit(null)">
            <mat-icon>add</mat-icon>Ajouter un emplacement
          </button>
        }
      </section>

      <app-ai-settings />
      <app-photo-settings />
      <app-off-settings />

      <section class="card">
        <h2 class="widget-title">Péremption</h2>
        <form class="row" (ngSubmit)="saveSettings()">
          <mat-form-field>
            <mat-label>Seuil « bientôt » général</mat-label>
            <input
              matInput
              name="soonDays"
              type="number"
              min="0"
              max="365"
              [ngModel]="soonDays()"
              (ngModelChange)="soonDays.set($event)"
            />
            <span matTextSuffix>jours</span>
            <mat-hint>Un emplacement peut avoir le sien.</mat-hint>
          </mat-form-field>
          <button mat-flat-button type="submit">Enregistrer</button>
        </form>
      </section>

      <section class="card">
        <h2 class="widget-title">Données</h2>
        <p class="muted">
          Export complet au format JSON, réimportable (ADR-0008).
          @if (info.value(); as i) {
            Version {{ i.version }}, schéma {{ i.schema_version }}.
            @if (i.last_backup; as b) {
              Dernière sauvegarde : {{ b.at.slice(0, 10) | frDate }}.
            }
          }
        </p>
        <div class="actions">
          <a mat-stroked-button href="/api/v1/export" download
            ><mat-icon>download</mat-icon>Exporter</a
          >
          <button mat-button (click)="logoutEverywhere()">
            <mat-icon>devices</mat-icon>Déconnecter tous les appareils
          </button>
        </div>
      </section>
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .line {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    .grow {
      flex: 1;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: var(--field-gap);
      margin-top: 0.5rem;
    }
  `,
})
export class SettingsPage {
  private readonly stock = inject(StockService);
  private readonly system = inject(SystemService);
  private readonly version = inject(DataVersion);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly session = inject(Session);
  protected readonly locations = inject(LocationsStore);

  protected readonly kinds: LocationKind[] = ['PANTRY', 'FRIDGE', 'FREEZER'];
  protected readonly kindLabels = LOCATION_KIND_LABELS;
  protected readonly draft = signal<LocationDraft | null>(null);
  protected readonly soonDays = signal(3);

  protected readonly settings = rxResource({ stream: () => this.system.getSettings() });
  protected readonly info = rxResource({ stream: () => this.system.systemInfo() });

  constructor() {
    effect(() => {
      if (this.settings.hasValue()) this.soonDays.set(this.settings.value().soon_days);
    });
  }

  protected edit(location: Location | null): void {
    this.draft.set(
      location
        ? {
            id: location.id,
            name: location.name,
            kind: location.kind,
            soon_days: location.soon_days ?? null,
          }
        : { id: null, name: '', kind: 'PANTRY', soon_days: null },
    );
  }

  protected patch(change: Partial<LocationDraft>): void {
    this.draft.update((d) => (d ? { ...d, ...change } : d));
  }

  protected async saveLocation(d: LocationDraft): Promise<void> {
    const body = {
      name: d.name,
      kind: d.kind,
      soon_days: d.soon_days === null ? null : Number(d.soon_days),
    };
    const ok = await this.run(
      d.id ? this.stock.updateLocation({ id: d.id, body }) : this.stock.createLocation({ body }),
      'Emplacement enregistré',
    );
    if (ok) this.draft.set(null);
  }

  protected async remove(location: Location): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: `Supprimer ${location.name} ?`,
      message: "Seul un emplacement qui n'a jamais contenu de lot peut être supprimé.",
      action: 'Supprimer',
      destructive: true,
    });
    if (ok) await this.run(this.stock.deleteLocation({ id: location.id }), 'Emplacement supprimé');
  }

  protected async saveSettings(): Promise<void> {
    await this.run(
      this.system.updateSettings({ body: { soon_days: Number(this.soonDays()) } }),
      'Réglages enregistrés',
    );
  }

  protected async logoutEverywhere(): Promise<void> {
    await this.session.logout(true);
    location.assign('/connexion');
  }

  private async run(request: Observable<unknown>, message: string): Promise<boolean> {
    try {
      await firstValueFrom(request);
      this.version.bump();
      this.snack.open(message, undefined, { duration: 3000 });
      return true;
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
      return false;
    }
  }
}
