import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatDialog } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatSnackBar } from '@angular/material/snack-bar';
import { Observable, firstValueFrom } from 'rxjs';

import { Criterion } from '../../api/models/criterion';
import { CriterionKind } from '../../api/models/criterion-kind';
import { GoalVersion } from '../../api/models/goal-version';
import { MealSlot } from '../../api/models/meal-slot';
import { GoalsService } from '../../api/services/goals.service';
import { fromIsoDate, toIsoDate } from '../../core/dates';
import { errorMessage } from '../../core/http-errors';
import {
  CRITERION_KIND_LABELS,
  IMPORTANCE_LABELS,
  NUTRITION_GROUPS,
  NUTRITION_GROUP_LABELS,
  PERIOD_LABELS,
} from '../../core/labels';
import { NUTRIENTS } from '../../core/nutrients';
import { confirm } from '../../shared/confirm-dialog';
import { FrDatePipe } from '../../shared/date.pipe';
import { GroupsStore } from '../../shared/stores';

/** Facteur d'affichage et unité des bornes d'un critère (valeurs entières côté serveur, ADR-0001). */
function scale(c: Criterion): { factor: number; unit: string } {
  switch (c.kind) {
    case 'ENERGY':
      return { factor: 1000, unit: 'kcal' };
    case 'NUTRIENT':
      return c.expression === 'PERCENT_ENERGY'
        ? { factor: 10, unit: '% de l’énergie' }
        : { factor: 1000, unit: 'g' };
    case 'GROUP_PORTIONS':
      return { factor: 1000, unit: 'portions' };
    case 'GROUP_AMOUNT':
      return { factor: 1000, unit: 'g' };
    case 'GROUP_FREQUENCY':
      return { factor: 1, unit: 'repas' };
    case 'RECIPE_REPEAT':
      return { factor: 1, unit: 'jours d’écart' };
    case 'FAMILY_REPEAT':
      return { factor: 1, unit: 'fois' };
  }
}

const KINDS = Object.keys(CRITERION_KIND_LABELS) as CriterionKind[];

/** Un critère vide du type demandé, prêt à compléter. */
function blank(kind: CriterionKind): Criterion {
  return {
    kind,
    expression: 'ABSOLUTE',
    period: kind === 'ENERGY' || kind === 'NUTRIENT' || kind === 'GROUP_PORTIONS' ? 'DAY' : 'WEEK',
    importance: 'PREFERRED',
    nutrient_code: kind === 'ENERGY' ? 'ENERGY' : kind === 'NUTRIENT' ? 'PROTEIN' : null,
    nutrition_groups: [],
    tolerance: kind === 'ENERGY' ? 100 : null,
    occurrence_threshold: kind === 'GROUP_FREQUENCY' ? 1000 : null,
  };
}

/**
 * Objectifs nutritionnels (NUT-01 à NUT-03) : versions datées et leurs critères, modèles, créneaux de repas.
 * Une version entrée en vigueur ne se modifie plus (R-NUT-3) : on en crée une nouvelle.
 */
@Component({
  selector: 'app-goals',
  imports: [
    FormsModule,
    MatButtonModule,
    MatDatepickerModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatProgressBarModule,
    MatSelectModule,
    MatSlideToggleModule,
    FrDatePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header">
        <h1>Objectifs nutritionnels</h1>
      </div>
      @if (versions.isLoading()) {
        <mat-progress-bar mode="indeterminate" />
      }

      <section class="card">
        <h2 class="widget-title">Versions</h2>
        @for (v of versions.value() ?? []; track v.id) {
          <button
            class="version"
            type="button"
            [class.selected]="v.id === selectedId()"
            (click)="select(v)"
          >
            <span>
              Depuis le {{ v.valid_from | frDate }}
              @if (v.current) {
                <span class="status">en vigueur</span>
              } @else if (!v.locked) {
                <span class="status soon">à venir</span>
              }
            </span>
            <span class="meta"
              >{{ v.criteria.length }} critères{{ v.note ? ' · ' + v.note : '' }}</span
            >
          </button>
        } @empty {
          @if (versions.hasValue()) {
            <p class="muted">Aucun objectif. Créez-en un, à partir d'un modèle ou vide.</p>
          }
        }
        <form class="row new-version" (ngSubmit)="createVersion()">
          <mat-form-field>
            <mat-label>Nouvelle version à partir du</mat-label>
            <input
              matInput
              name="from"
              [matDatepicker]="fromPicker"
              [ngModel]="newFrom()"
              (ngModelChange)="newFrom.set($event)"
            />
            <mat-datepicker-toggle matIconSuffix [for]="fromPicker" />
            <mat-datepicker #fromPicker />
          </mat-form-field>
          <mat-form-field>
            <mat-label>Reprendre</mat-label>
            <mat-select
              name="source"
              [ngModel]="newSource()"
              (ngModelChange)="newSource.set($event)"
            >
              <mat-option value="">Rien (version vide)</mat-option>
              @if (selected(); as s) {
                <mat-option [value]="'version:' + s.id"
                  >La version du {{ s.valid_from | frDate }}</mat-option
                >
              }
              @for (t of templates.value() ?? []; track t.id) {
                <mat-option [value]="'template:' + t.id">Modèle « {{ t.name }} »</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <button mat-flat-button type="submit" [disabled]="!newFrom()">
            <mat-icon>add</mat-icon>Créer
          </button>
        </form>
      </section>

      @if (selected(); as s) {
        <section class="card">
          <div class="section-head">
            <h2 class="widget-title">Critères de la version du {{ s.valid_from | frDate }}</h2>
            @if (s.locked) {
              <span class="muted">En vigueur : non modifiable, créez une nouvelle version.</span>
            }
          </div>
          @for (c of criteria(); track $index; let i = $index) {
            <div class="criterion" [class.locked]="s.locked">
              <div class="row">
                <mat-form-field>
                  <mat-label>Critère</mat-label>
                  <mat-select
                    [disabled]="s.locked"
                    [ngModel]="c.kind"
                    (ngModelChange)="replace(i, blankOf($event))"
                  >
                    @for (k of kinds; track k) {
                      <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
                @if (c.kind === 'NUTRIENT') {
                  <mat-form-field>
                    <mat-label>Nutriment</mat-label>
                    <mat-select
                      [disabled]="s.locked"
                      [ngModel]="c.nutrient_code"
                      (ngModelChange)="patch(i, { nutrient_code: $event })"
                    >
                      @for (n of nutrients; track n.code) {
                        @if (n.code !== 'ENERGY') {
                          <mat-option [value]="n.code">{{ n.label }}</mat-option>
                        }
                      }
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field>
                    <mat-label>Exprimé en</mat-label>
                    <mat-select
                      [disabled]="s.locked"
                      [ngModel]="c.expression"
                      (ngModelChange)="
                        patch(i, { expression: $event, min_value: null, max_value: null })
                      "
                    >
                      <mat-option value="ABSOLUTE">grammes</mat-option>
                      <mat-option value="PERCENT_ENERGY">part de l'énergie</mat-option>
                    </mat-select>
                  </mat-form-field>
                }
                @if (
                  c.kind === 'GROUP_PORTIONS' ||
                  c.kind === 'GROUP_AMOUNT' ||
                  c.kind === 'GROUP_FREQUENCY'
                ) {
                  <mat-form-field class="wide">
                    <mat-label>Groupes nutritionnels</mat-label>
                    <mat-select
                      multiple
                      [disabled]="s.locked"
                      [ngModel]="c.nutrition_groups"
                      (ngModelChange)="patch(i, { nutrition_groups: $event })"
                    >
                      @for (g of groups; track g) {
                        <mat-option [value]="g">{{ groupLabels[g] }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                }
                @if (c.kind === 'FAMILY_REPEAT') {
                  <mat-form-field class="wide">
                    <mat-label>Famille d'aliments</mat-label>
                    <mat-select
                      [disabled]="s.locked"
                      [ngModel]="c.food_group_id"
                      (ngModelChange)="patch(i, { food_group_id: $event })"
                    >
                      @for (g of foodGroups.list(); track g.id) {
                        <mat-option [value]="g.id">{{ g.name }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                }
                @if (!s.locked) {
                  <button
                    mat-icon-button
                    type="button"
                    aria-label="Retirer le critère"
                    (click)="remove(i)"
                  >
                    <mat-icon>delete</mat-icon>
                  </button>
                }
              </div>
              <div class="row">
                <mat-form-field class="short">
                  <mat-label>Minimum</mat-label>
                  <input
                    matInput
                    type="number"
                    min="0"
                    [disabled]="s.locked"
                    [ngModel]="shown(c, c.min_value)"
                    (ngModelChange)="patch(i, { min_value: stored(c, $event) })"
                  />
                  <span matTextSuffix>{{ unitOf(c) }}</span>
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Maximum</mat-label>
                  <input
                    matInput
                    type="number"
                    min="0"
                    [disabled]="s.locked"
                    [ngModel]="shown(c, c.max_value)"
                    (ngModelChange)="patch(i, { max_value: stored(c, $event) })"
                  />
                  <span matTextSuffix>{{ unitOf(c) }}</span>
                </mat-form-field>
                @if (c.kind === 'ENERGY') {
                  <mat-form-field class="short">
                    <mat-label>Tolérance</mat-label>
                    <input
                      matInput
                      type="number"
                      min="0"
                      [disabled]="s.locked"
                      [ngModel]="scaledDown(c.tolerance, 10)"
                      (ngModelChange)="patch(i, { tolerance: scaledUp($event, 10) })"
                    />
                    <span matTextSuffix>%</span>
                  </mat-form-field>
                }
                @if (c.kind === 'GROUP_FREQUENCY') {
                  <mat-form-field class="short">
                    <mat-label>Compte dès</mat-label>
                    <input
                      matInput
                      type="number"
                      min="0"
                      step="0.5"
                      [disabled]="s.locked"
                      [ngModel]="scaledDown(c.occurrence_threshold, 1000)"
                      (ngModelChange)="patch(i, { occurrence_threshold: scaledUp($event, 1000) })"
                    />
                    <span matTextSuffix>portion</span>
                  </mat-form-field>
                }
                <mat-form-field class="short">
                  <mat-label>Période</mat-label>
                  <mat-select
                    [disabled]="s.locked"
                    [ngModel]="c.period"
                    (ngModelChange)="patch(i, { period: $event })"
                  >
                    <mat-option value="DAY">{{ periodLabels.DAY }}</mat-option>
                    <mat-option value="WEEK">{{ periodLabels.WEEK }}</mat-option>
                  </mat-select>
                </mat-form-field>
                <mat-form-field class="short">
                  <mat-label>Importance</mat-label>
                  <mat-select
                    [disabled]="s.locked"
                    [ngModel]="c.importance"
                    (ngModelChange)="patch(i, { importance: $event })"
                  >
                    <mat-option value="REQUIRED">{{ importanceLabels.REQUIRED }}</mat-option>
                    <mat-option value="PREFERRED">{{ importanceLabels.PREFERRED }}</mat-option>
                    <mat-option value="INFO">{{ importanceLabels.INFO }}</mat-option>
                  </mat-select>
                </mat-form-field>
              </div>
            </div>
          } @empty {
            <p class="muted">Aucun critère.</p>
          }
          <div class="actions">
            @if (!s.locked) {
              <button mat-stroked-button type="button" (click)="add()">
                <mat-icon>add</mat-icon>Critère
              </button>
              <button mat-button type="button" class="destructive" (click)="deleteVersion(s)">
                Supprimer la version
              </button>
              <span class="spacer"></span>
              <button mat-flat-button type="button" [disabled]="!dirty()" (click)="saveVersion(s)">
                Enregistrer
              </button>
            } @else {
              <span class="spacer"></span>
            }
            <button mat-stroked-button type="button" (click)="saveAsTemplate()">
              <mat-icon>bookmark_add</mat-icon>Enregistrer comme modèle
            </button>
          </div>
        </section>
      }

      @if (customTemplates().length) {
        <section class="card">
          <h2 class="widget-title">Mes modèles</h2>
          @for (t of customTemplates(); track t.id) {
            <div class="template">
              <span
                >{{ t.name }} <span class="meta">· {{ t.criteria.length }} critères</span></span
              >
              <button
                mat-icon-button
                type="button"
                aria-label="Supprimer le modèle"
                (click)="deleteTemplate(t.id, t.name)"
              >
                <mat-icon>delete</mat-icon>
              </button>
            </div>
          }
        </section>
      }

      <section class="card">
        <h2 class="widget-title">Créneaux de repas</h2>
        <p class="muted">
          Part de l'énergie de la journée apportée par chaque repas actif ; le total des créneaux
          actifs fait 100 %.
        </p>
        @for (slot of slots(); track $index; let i = $index) {
          <div class="row">
            <mat-form-field>
              <mat-label>Repas</mat-label>
              <input
                matInput
                [ngModel]="slot.name"
                (ngModelChange)="patchSlot(i, { name: $event })"
              />
            </mat-form-field>
            <mat-form-field class="short">
              <mat-label>Part</mat-label>
              <input
                matInput
                type="number"
                min="0"
                max="100"
                [ngModel]="slot.energy_share / 10"
                (ngModelChange)="patchSlot(i, { energy_share: Math.round(($event || 0) * 10) })"
              />
              <span matTextSuffix>%</span>
            </mat-form-field>
            <mat-slide-toggle
              [ngModel]="slot.active"
              (ngModelChange)="patchSlot(i, { active: $event })"
              >Actif</mat-slide-toggle
            >
          </div>
        }
        <div class="actions">
          <span [class.error]="slotTotal() !== 1000">Total : {{ slotTotal() / 10 }} %</span>
          <span class="spacer"></span>
          <button
            mat-flat-button
            type="button"
            [disabled]="!slotsDirty() || slotTotal() !== 1000"
            (click)="saveSlots()"
          >
            Enregistrer
          </button>
        </div>
      </section>
    </div>
  `,
  styles: `
    .card {
      margin-bottom: 1rem;
    }
    .version {
      display: flex;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 0.5rem;
      width: 100%;
      padding: 0.75rem 1rem;
      border: 0;
      border-radius: var(--mat-sys-corner-medium);
      background: none;
      color: inherit;
      font: var(--mat-sys-body-large);
      text-align: left;
      cursor: pointer;
    }
    .version:hover {
      background: var(--mat-sys-surface-container);
    }
    .version.selected {
      background: var(--mat-sys-secondary-container);
    }
    .version .status {
      margin-left: 0.5rem;
    }
    .meta {
      font: var(--mat-sys-body-small);
      color: var(--mat-sys-on-surface-variant);
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0 var(--field-gap);
    }
    .new-version {
      margin-top: 1rem;
    }
    .short {
      width: 10rem;
    }
    .wide {
      min-width: 16rem;
      flex: 1;
    }
    .criterion {
      padding: 0.75rem 0 0;
      border-top: 1px solid var(--mat-sys-outline-variant);
    }
    .section-head {
      display: flex;
      flex-wrap: wrap;
      align-items: baseline;
      gap: 0.5rem 1rem;
    }
    .actions {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.5rem;
      margin-top: 0.5rem;
    }
    .spacer {
      flex: 1;
    }
    .template {
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    .error {
      color: var(--mat-sys-error);
    }
  `,
})
export class GoalsPage {
  private readonly api = inject(GoalsService);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  protected readonly foodGroups = inject(GroupsStore);

  protected readonly Math = Math;
  protected readonly kinds = KINDS;
  protected readonly kindLabels = CRITERION_KIND_LABELS;
  protected readonly importanceLabels = IMPORTANCE_LABELS;
  protected readonly periodLabels = PERIOD_LABELS;
  protected readonly groups = NUTRITION_GROUPS;
  protected readonly groupLabels = NUTRITION_GROUP_LABELS;
  protected readonly nutrients = NUTRIENTS;
  protected readonly blankOf = blank;

  protected readonly versions = rxResource({ stream: () => this.api.listGoalVersions() });
  protected readonly templates = rxResource({ stream: () => this.api.listGoalTemplates() });
  private readonly savedSlots = rxResource({ stream: () => this.api.listMealSlots() });

  protected readonly selectedId = signal<string | null>(null);
  protected readonly selected = computed(() => {
    const list = this.versions.value() ?? [];
    return (
      list.find((v) => v.id === this.selectedId()) ?? list.find((v) => v.current) ?? list[0] ?? null
    );
  });
  protected readonly criteria = signal<Criterion[]>([]);
  protected readonly dirty = signal(false);

  protected readonly newFrom = signal<Date | null>(null);
  protected readonly newSource = signal('');

  protected readonly customTemplates = computed(() =>
    (this.templates.value() ?? []).filter((t) => !t.builtin),
  );

  protected readonly slots = signal<MealSlot[]>([]);
  protected readonly slotsDirty = signal(false);
  protected readonly slotTotal = computed(() =>
    this.slots()
      .filter((s) => s.active)
      .reduce((sum, s) => sum + s.energy_share, 0),
  );

  constructor() {
    effect(() => {
      this.criteria.set(structuredClone(this.selected()?.criteria ?? []));
      this.dirty.set(false);
    });
    effect(() => {
      this.slots.set(structuredClone(this.savedSlots.value() ?? []));
      this.slotsDirty.set(false);
    });
  }

  protected select(version: GoalVersion): void {
    this.selectedId.set(version.id);
  }

  protected unitOf(c: Criterion): string {
    return scale(c).unit;
  }

  protected shown(c: Criterion, raw: number | null | undefined): number | null {
    return this.scaledDown(raw, scale(c).factor);
  }

  protected stored(c: Criterion, value: number | string | null): number | null {
    return this.scaledUp(value, scale(c).factor);
  }

  /** Valeur stockée → valeur saisie (3 200 → 3,2 pour un facteur de 1 000). */
  protected scaledDown(raw: number | null | undefined, factor: number): number | null {
    return raw == null ? null : raw / factor;
  }

  /** Valeur saisie → valeur stockée, entière ; vide → `null`. */
  protected scaledUp(value: number | string | null, factor: number): number | null {
    return value == null || value === '' ? null : Math.round(Number(value) * factor);
  }

  protected replace(index: number, criterion: Criterion): void {
    this.criteria.update((list) => list.map((c, i) => (i === index ? criterion : c)));
    this.dirty.set(true);
  }

  protected patch(index: number, change: Partial<Criterion>): void {
    this.criteria.update((list) => list.map((c, i) => (i === index ? { ...c, ...change } : c)));
    this.dirty.set(true);
  }

  protected add(): void {
    this.criteria.update((list) => [...list, blank('NUTRIENT')]);
    this.dirty.set(true);
  }

  protected remove(index: number): void {
    this.criteria.update((list) => list.filter((_, i) => i !== index));
    this.dirty.set(true);
  }

  protected patchSlot(index: number, change: Partial<MealSlot>): void {
    this.slots.update((list) => list.map((s, i) => (i === index ? { ...s, ...change } : s)));
    this.slotsDirty.set(true);
  }

  protected async createVersion(): Promise<void> {
    const from = this.newFrom();
    if (!from) {
      return;
    }
    const [kind, id] = this.newSource().split(':');
    const created = await this.run(
      this.api.createGoalVersion({
        body: {
          valid_from: toIsoDate(from),
          copy_from: kind === 'version' ? id : null,
          template_id: kind === 'template' ? id : null,
        },
      }),
      'Version créée',
    );
    if (created) {
      this.newFrom.set(null);
      this.versions.reload();
      this.selectedId.set(created.id);
    }
  }

  protected async saveVersion(version: GoalVersion): Promise<void> {
    const body = { valid_from: version.valid_from, note: version.note, criteria: this.criteria() };
    if (
      (await this.run(
        this.api.updateGoalVersion({ id: version.id, body }),
        'Objectif enregistré',
      )) !== undefined
    ) {
      this.versions.reload();
    }
  }

  protected async deleteVersion(version: GoalVersion): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer la version',
      message: `La version du ${fromIsoDate(version.valid_from).toLocaleDateString('fr-FR')} sera supprimée.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (
      ok &&
      (await this.run(this.api.deleteGoalVersion({ id: version.id }), 'Version supprimée')) !==
        undefined
    ) {
      this.selectedId.set(null);
      this.versions.reload();
    }
  }

  protected async saveAsTemplate(): Promise<void> {
    const name = prompt('Nom du modèle')?.trim();
    if (name) {
      const body = { name, criteria: this.criteria() };
      if (
        (await this.run(this.api.createGoalTemplate({ body }), 'Modèle enregistré')) !== undefined
      ) {
        this.templates.reload();
      }
    }
  }

  protected async deleteTemplate(id: string, name: string): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le modèle',
      message: `Le modèle « ${name} » sera supprimé.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (
      ok &&
      (await this.run(this.api.deleteGoalTemplate({ id }), 'Modèle supprimé')) !== undefined
    ) {
      this.templates.reload();
    }
  }

  protected async saveSlots(): Promise<void> {
    if (
      (await this.run(this.api.saveMealSlots({ body: this.slots() }), 'Créneaux enregistrés')) !==
      undefined
    ) {
      this.savedSlots.reload();
    }
  }

  /** Lance un appel ; `undefined` en cas d'erreur (affichée), sinon sa réponse (ou `null`). */
  private async run<T>(call: Observable<T>, message: string): Promise<T | null | undefined> {
    try {
      const result = await firstValueFrom(call, { defaultValue: null });
      this.snack.open(message, undefined, { duration: 3000 });
      return result;
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
      return undefined;
    }
  }
}
