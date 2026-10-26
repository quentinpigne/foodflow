import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal,
  untracked,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatDialog } from '@angular/material/dialog';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatIconModule } from '@angular/material/icon';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSnackBar } from '@angular/material/snack-bar';
import { MatTabsModule } from '@angular/material/tabs';
import { Observable, firstValueFrom } from 'rxjs';

import { DaySchedule } from '../../api/models/day-schedule';
import { HabitualItem } from '../../api/models/habitual-item';
import { HabitualMeal } from '../../api/models/habitual-meal';
import { Member } from '../../api/models/member';
import { PreferenceLevel } from '../../api/models/preference-level';
import { PreferenceView } from '../../api/models/preference-view';
import { RecipeKind } from '../../api/models/recipe-kind';
import { SlotRole } from '../../api/models/slot-role';
import { SlotTemplate } from '../../api/models/slot-template';
import { PlanningService } from '../../api/services/planning.service';
import { DataVersion } from '../../core/data-version';
import { errorMessage } from '../../core/http-errors';
import {
  NUTRITION_GROUPS,
  NUTRITION_GROUP_LABELS,
  PREFERENCE_LABELS,
  RECIPE_KINDS,
  RECIPE_KIND_LABELS,
  WEEKDAYS,
} from '../../core/labels';
import {
  formatPortions,
  formatQuantity,
  parsePortions,
  parseQuantity,
  quantityInput,
} from '../../core/quantity';
import { confirm } from '../../shared/confirm-dialog';
import { ItemPicker, StockItem, itemName } from '../../shared/item-picker';
import { PickedTarget, TargetPicker } from '../../shared/target-picker';

interface MemberForm {
  id: string | null;
  name: string;
  coefficient: string;
  active: boolean;
  /** « créneau|jour » des absences. */
  absences: Set<string>;
  preferences: PreferenceView[];
}

interface HabitualForm {
  id: string | null;
  name: string;
  items: (HabitualItem & { text: string })[];
}

const LEVELS: PreferenceLevel[] = ['LOVES', 'LIKES', 'DISLIKES', 'REFUSES'];

/** Foyer et semaine type (PLA-01, PLA-02, PLA-06 à PLA-08). */
@Component({
  selector: 'app-household',
  imports: [
    FormsModule,
    MatButtonModule,
    MatCheckboxModule,
    MatExpansionModule,
    MatFormFieldModule,
    MatIconModule,
    MatInputModule,
    MatSelectModule,
    MatTabsModule,
    ItemPicker,
    TargetPicker,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <div class="page-header"><h1>Foyer</h1></div>
      <mat-tab-group>
        <mat-tab label="Membres">
          <p class="muted intro">
            Le coefficient d'équivalent adulte règle les quantités : 1 pour un adulte, moins pour un
            enfant (0,6 vers 6 ans). Les absences par défaut retirent le membre des repas créés sur
            ces créneaux.
          </p>
          <mat-accordion>
            @for (f of memberForms(); track f.id ?? 'new'; let i = $index) {
              <mat-expansion-panel [expanded]="!f.id">
                <mat-expansion-panel-header>
                  <mat-panel-title>{{ f.name || 'Nouveau membre' }}</mat-panel-title>
                  <mat-panel-description>
                    {{ f.coefficient }} éq. adulte{{ f.active ? '' : ' · parti' }}
                  </mat-panel-description>
                </mat-expansion-panel-header>
                <div class="row">
                  <mat-form-field>
                    <mat-label>Nom</mat-label>
                    <input
                      matInput
                      [ngModel]="f.name"
                      (ngModelChange)="patchMember(i, { name: $event })"
                    />
                  </mat-form-field>
                  <mat-form-field class="short">
                    <mat-label>Équivalent adulte</mat-label>
                    <input
                      matInput
                      inputmode="decimal"
                      [ngModel]="f.coefficient"
                      (ngModelChange)="patchMember(i, { coefficient: $event })"
                    />
                  </mat-form-field>
                  <mat-checkbox
                    [ngModel]="f.active"
                    (ngModelChange)="patchMember(i, { active: $event })"
                    >Fait partie du foyer</mat-checkbox
                  >
                </div>
                <h3>Présent par défaut</h3>
                <div class="presence" [style.grid-template-columns]="'9rem repeat(7, 1fr)'">
                  <span></span>
                  @for (day of weekdays; track day) {
                    <span class="head">{{ day.slice(0, 3) }}.</span>
                  }
                  @for (slot of template.value() ?? []; track slot.slot_id) {
                    <span>{{ slot.name }}</span>
                    @for (day of weekdays; track day; let d = $index) {
                      <mat-checkbox
                        [checked]="!f.absences.has(slot.slot_id + '|' + (d + 1))"
                        (change)="togglePresence(i, slot.slot_id, d + 1, $event.checked)"
                        [attr.aria-label]="f.name + ' présent : ' + slot.name + ', ' + day"
                      />
                    }
                  }
                </div>
                <h3>Préférences</h3>
                @for (p of f.preferences; track $index; let pi = $index) {
                  <div class="row">
                    <span class="grow">{{ p.target_name }}</span>
                    <mat-form-field class="short">
                      <mat-label>Niveau</mat-label>
                      <mat-select
                        [ngModel]="p.level"
                        (ngModelChange)="patchPreference(i, pi, $event)"
                      >
                        @for (l of levels; track l) {
                          <mat-option [value]="l">{{ levelLabels[l] }}</mat-option>
                        }
                      </mat-select>
                    </mat-form-field>
                    <button
                      mat-icon-button
                      type="button"
                      aria-label="Retirer la préférence"
                      (click)="removePreference(i, pi)"
                    >
                      <mat-icon>close</mat-icon>
                    </button>
                  </div>
                }
                <app-target-picker
                  label="Ajouter une préférence"
                  (picked)="addPreference(i, $event)"
                />
                <div class="actions">
                  <button mat-flat-button type="button" (click)="saveMember(i)">Enregistrer</button>
                  @if (f.id) {
                    <button mat-button type="button" class="destructive" (click)="deleteMember(f)">
                      Retirer
                    </button>
                  }
                </div>
              </mat-expansion-panel>
            }
          </mat-accordion>
          @if (!hasNewMember()) {
            <button mat-stroked-button class="add" type="button" (click)="newMember()">
              <mat-icon>person_add</mat-icon>Membre
            </button>
          }
        </mat-tab>

        <mat-tab label="Semaine type">
          <p class="muted intro">
            Pour chaque créneau et chaque jour : le temps actif disponible pour cuisiner, l'heure à
            partir de laquelle on peut s'y mettre, l'heure où le repas doit être prêt. Les repas
            déjà créés gardent leurs valeurs.
          </p>
          @for (slot of slots(); track slot.slot_id; let si = $index) {
            <section class="card">
              <h2 class="widget-title">{{ slot.name }}</h2>
              <div class="schedule">
                <span class="head">Jour</span><span class="head">Temps actif</span
                ><span class="head">À partir de</span> <span class="head">Prêt à</span
                ><span class="head">Hors foyer</span><span class="head">Repas habituel</span>
                @for (day of slot.days; track day.weekday; let di = $index) {
                  <span>{{ weekdays[day.weekday - 1] }}</span>
                  <mat-form-field class="tiny">
                    <input
                      matInput
                      type="number"
                      min="0"
                      [attr.aria-label]="'Temps actif, ' + weekdays[day.weekday - 1]"
                      [ngModel]="day.available_active_minutes"
                      (ngModelChange)="patchDay(si, di, { available_active_minutes: $event ?? 0 })"
                    />
                    <span matTextSuffix>min</span>
                  </mat-form-field>
                  <mat-form-field class="tiny">
                    <input
                      matInput
                      type="time"
                      [attr.aria-label]="'À partir de, ' + weekdays[day.weekday - 1]"
                      [ngModel]="day.earliest_start"
                      (ngModelChange)="patchDay(si, di, { earliest_start: $event })"
                    />
                  </mat-form-field>
                  <mat-form-field class="tiny">
                    <input
                      matInput
                      type="time"
                      [attr.aria-label]="'Prêt à, ' + weekdays[day.weekday - 1]"
                      [ngModel]="day.ready_by"
                      (ngModelChange)="patchDay(si, di, { ready_by: $event })"
                    />
                  </mat-form-field>
                  <mat-checkbox
                    [ngModel]="day.out_of_home"
                    (ngModelChange)="patchDay(si, di, { out_of_home: $event })"
                    [attr.aria-label]="'Hors foyer, ' + weekdays[day.weekday - 1]"
                  />
                  <mat-form-field class="tiny">
                    <mat-select
                      [ngModel]="day.habitual_meal_id ?? null"
                      (ngModelChange)="patchDay(si, di, { habitual_meal_id: $event })"
                      [attr.aria-label]="'Repas habituel, ' + weekdays[day.weekday - 1]"
                    >
                      <mat-option [value]="null">—</mat-option>
                      @for (h of habitual.value() ?? []; track h.id) {
                        <mat-option [value]="h.id">{{ h.name }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                }
              </div>
              <h3>Composition type</h3>
              @for (role of slot.roles; track $index; let ri = $index) {
                <div class="row">
                  <mat-form-field>
                    <mat-label>Rôle</mat-label>
                    <input
                      matInput
                      [ngModel]="role.name"
                      (ngModelChange)="patchRole(si, ri, { name: $event })"
                    />
                  </mat-form-field>
                  <mat-form-field>
                    <mat-label>Recettes admises</mat-label>
                    <mat-select
                      multiple
                      [ngModel]="role.recipe_kinds ?? []"
                      (ngModelChange)="patchRole(si, ri, { recipe_kinds: $event })"
                    >
                      @for (k of recipeKinds; track k) {
                        <mat-option [value]="k">{{ kindLabels[k] }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field>
                    <mat-label>Ou un aliment de</mat-label>
                    <mat-select
                      multiple
                      [ngModel]="role.nutrition_groups ?? []"
                      (ngModelChange)="patchRole(si, ri, { nutrition_groups: $event })"
                    >
                      @for (g of groups; track g) {
                        <mat-option [value]="g">{{ groupLabels[g] }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                  <mat-checkbox
                    [ngModel]="role.required ?? false"
                    (ngModelChange)="patchRole(si, ri, { required: $event })"
                    >Obligatoire</mat-checkbox
                  >
                  <button
                    mat-icon-button
                    type="button"
                    aria-label="Retirer le rôle"
                    (click)="removeRole(si, ri)"
                  >
                    <mat-icon>delete</mat-icon>
                  </button>
                </div>
              }
              <div class="actions">
                <button mat-stroked-button type="button" (click)="addRole(si)">
                  <mat-icon>add</mat-icon>Rôle
                </button>
                <span class="spacer"></span>
                <button mat-flat-button type="button" (click)="saveSlot(slot)">Enregistrer</button>
              </div>
            </section>
          }
        </mat-tab>

        <mat-tab label="Repas habituels">
          <p class="muted intro">
            Un repas habituel (« Tartines ») se compose d'aliments et de recettes en quantité par
            équivalent adulte ; il est repris sur les créneaux qui le désignent dans la semaine
            type.
          </p>
          @for (h of habitualForms(); track h.id ?? 'new'; let hi = $index) {
            <section class="card">
              <div class="row">
                <mat-form-field class="grow">
                  <mat-label>Nom</mat-label>
                  <input
                    matInput
                    [ngModel]="h.name"
                    (ngModelChange)="patchHabitual(hi, { name: $event })"
                  />
                </mat-form-field>
              </div>
              @for (item of h.items; track $index; let ii = $index) {
                <div class="row">
                  <span class="grow">{{ item.name }}</span>
                  <mat-form-field class="short">
                    <mat-label>Par équivalent adulte</mat-label>
                    <input
                      matInput
                      [ngModel]="item.text"
                      (ngModelChange)="patchHabitualItem(hi, ii, $event)"
                    />
                  </mat-form-field>
                  <button
                    mat-icon-button
                    type="button"
                    aria-label="Retirer"
                    (click)="removeHabitualItem(hi, ii)"
                  >
                    <mat-icon>close</mat-icon>
                  </button>
                </div>
              }
              <app-item-picker label="Ajouter un aliment" (picked)="addHabitualFood(hi, $event)" />
              <div class="actions">
                <button mat-flat-button type="button" (click)="saveHabitual(hi)">
                  Enregistrer
                </button>
                @if (h.id) {
                  <button mat-button type="button" class="destructive" (click)="deleteHabitual(h)">
                    Supprimer
                  </button>
                }
              </div>
            </section>
          }
          @if (!hasNewHabitual()) {
            <button mat-stroked-button class="add" type="button" (click)="newHabitual()">
              <mat-icon>add</mat-icon>Repas habituel
            </button>
          }
        </mat-tab>
      </mat-tab-group>
    </div>
  `,
  styles: `
    .intro {
      margin: 1rem 0;
    }
    .card {
      margin-bottom: 1rem;
    }
    h3 {
      font: var(--mat-sys-title-small);
      margin: 1.25rem 0 0.5rem;
    }
    .row {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 0.75rem var(--field-gap);
    }
    .short {
      width: 11rem;
    }
    .grow {
      flex: 1;
      min-width: 12rem;
    }
    .presence {
      display: grid;
      align-items: center;
      gap: 0.25rem;
      overflow-x: auto;
    }
    .head {
      font: var(--mat-sys-label-medium);
      color: var(--mat-sys-on-surface-variant);
    }
    .schedule {
      display: grid;
      grid-template-columns: 6rem 8rem 8rem 8rem 6rem minmax(8rem, 1fr);
      align-items: center;
      gap: 0.25rem 0.75rem;
      overflow-x: auto;
    }
    .tiny {
      width: 100%;
    }
    .actions {
      margin-top: 1rem;
      align-items: center;
    }
    .spacer {
      flex: 1;
    }
    .add {
      margin-top: 1rem;
    }
  `,
})
export class HouseholdPage {
  private readonly api = inject(PlanningService);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  private readonly version = inject(DataVersion);

  protected readonly weekdays = WEEKDAYS;
  protected readonly levels = LEVELS;
  protected readonly levelLabels = PREFERENCE_LABELS;
  protected readonly recipeKinds: RecipeKind[] = RECIPE_KINDS;
  protected readonly kindLabels = RECIPE_KIND_LABELS;
  protected readonly groups = NUTRITION_GROUPS;
  protected readonly groupLabels = NUTRITION_GROUP_LABELS;

  private readonly members = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listMembers(),
  });
  protected readonly template = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.getWeekTemplate(),
  });
  protected readonly habitual = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listHabitualMeals(),
  });

  protected readonly memberForms = signal<MemberForm[]>([]);
  protected readonly slots = signal<SlotTemplate[]>([]);
  protected readonly habitualForms = signal<HabitualForm[]>([]);
  protected readonly hasNewMember = computed(() => this.memberForms().some((f) => !f.id));
  protected readonly hasNewHabitual = computed(() => this.habitualForms().some((f) => !f.id));

  constructor() {
    effect(() => {
      const members = this.members.value();
      if (members) untracked(() => this.memberForms.set(members.map((m) => this.memberForm(m))));
    });
    effect(() => {
      const template = this.template.value();
      if (template) untracked(() => this.slots.set(structuredClone(template)));
    });
    effect(() => {
      const habitual = this.habitual.value();
      if (habitual)
        untracked(() => this.habitualForms.set(habitual.map((h) => this.habitualForm(h))));
    });
  }

  // ----- Membres

  private memberForm(m: Member): MemberForm {
    return {
      id: m.id,
      name: m.name,
      coefficient: formatPortions(m.adult_equivalent),
      active: m.active,
      absences: new Set(m.absences.map((a) => `${a.slot_id}|${a.weekday}`)),
      preferences: m.preferences,
    };
  }

  protected newMember(): void {
    this.memberForms.update((f) => [
      ...f,
      { id: null, name: '', coefficient: '1', active: true, absences: new Set(), preferences: [] },
    ]);
  }

  protected patchMember(index: number, change: Partial<MemberForm>): void {
    this.memberForms.update((f) => f.map((x, i) => (i === index ? { ...x, ...change } : x)));
  }

  protected togglePresence(index: number, slot: string, weekday: number, present: boolean): void {
    const absences = new Set(this.memberForms()[index].absences);
    const key = `${slot}|${weekday}`;
    if (present) absences.delete(key);
    else absences.add(key);
    this.patchMember(index, { absences });
  }

  protected addPreference(index: number, picked: PickedTarget): void {
    const preferences = this.memberForms()[index].preferences.filter(
      (p) => JSON.stringify(p.target) !== JSON.stringify(picked.target),
    );
    this.patchMember(index, {
      preferences: [
        ...preferences,
        { target: picked.target, target_name: picked.name, level: 'REFUSES' },
      ],
    });
  }

  protected patchPreference(index: number, pref: number, level: PreferenceLevel): void {
    const preferences = this.memberForms()[index].preferences.map((p, i) =>
      i === pref ? { ...p, level } : p,
    );
    this.patchMember(index, { preferences });
  }

  protected removePreference(index: number, pref: number): void {
    this.patchMember(index, {
      preferences: this.memberForms()[index].preferences.filter((_, i) => i !== pref),
    });
  }

  protected async saveMember(index: number): Promise<void> {
    const f = this.memberForms()[index];
    const coefficient = parsePortions(f.coefficient);
    if (!coefficient) {
      this.snack.open('Équivalent adulte invalide (1, 0,6…).', 'Fermer');
      return;
    }
    const body = {
      name: f.name,
      adult_equivalent: coefficient,
      active: f.active,
      absences: [...f.absences].map((key) => {
        const [slot_id, weekday] = key.split('|');
        return { slot_id, weekday: Number(weekday) };
      }),
      preferences: f.preferences.map((p) => ({ target: p.target, level: p.level })),
    };
    await this.run(
      f.id ? this.api.updateMember({ id: f.id, body }) : this.api.createMember({ body }),
      'Membre enregistré',
    );
  }

  protected async deleteMember(f: MemberForm): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Retirer du foyer',
      message: `${f.name} est retiré du foyer ; s'il a déjà été convive, il reste dans l'historique.`,
      action: 'Retirer',
      destructive: true,
    });
    if (ok && f.id) await this.run(this.api.deleteMember({ id: f.id }), 'Membre retiré');
  }

  // ----- Semaine type

  protected patchDay(slot: number, day: number, change: Partial<DaySchedule>): void {
    this.slots.update((s) =>
      s.map((x, i) =>
        i === slot
          ? { ...x, days: x.days.map((d, j) => (j === day ? { ...d, ...change } : d)) }
          : x,
      ),
    );
  }

  protected patchRole(slot: number, role: number, change: Partial<SlotRole>): void {
    this.slots.update((s) =>
      s.map((x, i) =>
        i === slot
          ? { ...x, roles: x.roles.map((r, j) => (j === role ? { ...r, ...change } : r)) }
          : x,
      ),
    );
  }

  protected addRole(slot: number): void {
    this.slots.update((s) =>
      s.map((x, i) =>
        i === slot ? { ...x, roles: [...x.roles, { name: '', required: false }] } : x,
      ),
    );
  }

  protected removeRole(slot: number, role: number): void {
    this.slots.update((s) =>
      s.map((x, i) => (i === slot ? { ...x, roles: x.roles.filter((_, j) => j !== role) } : x)),
    );
  }

  protected async saveSlot(slot: SlotTemplate): Promise<void> {
    await this.run(
      this.api.saveSlotTemplate({
        slot_id: slot.slot_id,
        body: { days: slot.days, roles: slot.roles },
      }),
      `${slot.name} enregistré`,
    );
  }

  // ----- Repas habituels

  private habitualForm(h: HabitualMeal): HabitualForm {
    return {
      id: h.id,
      name: h.name,
      items: h.items.map((i) => ({
        ...i,
        text: i.measure ? quantityInput(i.amount, i.measure) : formatPortions(i.amount),
      })),
    };
  }

  protected newHabitual(): void {
    this.habitualForms.update((f) => [...f, { id: null, name: '', items: [] }]);
  }

  protected patchHabitual(index: number, change: Partial<HabitualForm>): void {
    this.habitualForms.update((f) => f.map((x, i) => (i === index ? { ...x, ...change } : x)));
  }

  protected patchHabitualItem(index: number, item: number, text: string): void {
    const items = this.habitualForms()[index].items.map((x, i) =>
      i === item ? { ...x, text } : x,
    );
    this.patchHabitual(index, { items });
  }

  protected removeHabitualItem(index: number, item: number): void {
    this.patchHabitual(index, {
      items: this.habitualForms()[index].items.filter((_, i) => i !== item),
    });
  }

  protected addHabitualFood(index: number, picked: StockItem): void {
    const measure =
      picked.kind === 'food' ? picked.food.effective.stock_measure : picked.product.stock_measure;
    const item = {
      food_id: picked.kind === 'food' ? picked.food.id : null,
      product_id: picked.kind === 'product' ? picked.product.id : null,
      recipe_id: null,
      amount: 0,
      name: itemName(picked),
      measure,
      text: '',
    };
    this.patchHabitual(index, { items: [...this.habitualForms()[index].items, item] });
  }

  protected async saveHabitual(index: number): Promise<void> {
    const f = this.habitualForms()[index];
    const items: HabitualItem[] = [];
    for (const i of f.items) {
      const amount = i.measure ? parseQuantity(i.text, i.measure) : parsePortions(i.text);
      if (!amount) {
        const unit = i.measure ? formatQuantity(1000, i.measure).split('\u00a0')[1] : 'portions';
        this.snack.open(`Quantité invalide pour ${i.name} (en ${unit}).`, 'Fermer');
        return;
      }
      items.push({ food_id: i.food_id, product_id: i.product_id, recipe_id: i.recipe_id, amount });
    }
    const body = { name: f.name, items };
    await this.run(
      f.id
        ? this.api.updateHabitualMeal({ id: f.id, body })
        : this.api.createHabitualMeal({ body }),
      'Repas habituel enregistré',
    );
  }

  protected async deleteHabitual(h: HabitualForm): Promise<void> {
    const ok = await confirm(this.dialog, {
      title: 'Supprimer le repas habituel',
      message: `« ${h.name} » n'est plus proposé ; les repas déjà créés le gardent.`,
      action: 'Supprimer',
      destructive: true,
    });
    if (ok && h.id)
      await this.run(this.api.deleteHabitualMeal({ id: h.id }), 'Repas habituel supprimé');
  }

  private async run(call: Observable<unknown>, message: string): Promise<void> {
    try {
      await firstValueFrom(call, { defaultValue: null });
      this.snack.open(message, undefined, { duration: 2500 });
      this.version.bump();
    } catch (error) {
      this.snack.open(errorMessage(error), 'Fermer');
    }
  }
}
