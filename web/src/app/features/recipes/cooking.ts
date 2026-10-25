import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  input,
  signal,
} from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { RouterLink } from '@angular/router';

import { RecipesService } from '../../api/services/recipes.service';
import { parseQuantity } from '../../core/quantity';
import { quantityText } from './ingredients';

interface Timer {
  step: number;
  label: string;
  endsAt: number;
}

/**
 * Mode cuisine (REC-08) : une étape à la fois, en grand, les quantités mises à l'échelle, des minuteurs, et
 * l'écran maintenu allumé (API Wake Lock) tant que la page est ouverte.
 */
@Component({
  selector: 'app-cooking',
  imports: [RouterLink, MatButtonModule, MatIconModule, MatProgressBarModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (recipe.hasValue()) {
      @let r = recipe.value();
      <div class="cooking">
        <header>
          <a mat-icon-button [routerLink]="['/recettes', r.id]" aria-label="Quitter le mode cuisine"
            ><mat-icon>close</mat-icon></a
          >
          <h1>{{ r.name }}</h1>
          <button mat-button (click)="showIngredients.set(!showIngredients())">
            <mat-icon>list</mat-icon>Ingrédients
          </button>
        </header>
        @if (showIngredients()) {
          <ul class="ingredients">
            @for (i of r.ingredients; track i.id) {
              <li>
                <strong>{{ quantityText(i) }}</strong>
                {{ i.target_name }}
                @if (i.note) {
                  <span class="muted">, {{ i.note }}</span>
                }
              </li>
            }
          </ul>
        }
        @if (r.steps.length) {
          @let s = r.steps[current()];
          <main>
            <div class="progress muted">
              Étape {{ current() + 1 }} / {{ r.steps.length
              }}{{ s.section ? ' · ' + s.section : '' }}
            </div>
            <p class="step">{{ s.text }}</p>
            @if (s.timer_minutes) {
              <button mat-flat-button (click)="startTimer(current(), s.text, s.timer_minutes)">
                <mat-icon>timer</mat-icon>Minuteur {{ s.timer_minutes }} min
              </button>
            }
          </main>
          <nav>
            <button
              mat-stroked-button
              [disabled]="current() === 0"
              (click)="current.set(current() - 1)"
            >
              <mat-icon>chevron_left</mat-icon>Précédente
            </button>
            <button
              mat-flat-button
              [disabled]="current() === r.steps.length - 1"
              (click)="current.set(current() + 1)"
            >
              Suivante<mat-icon iconPositionEnd>chevron_right</mat-icon>
            </button>
          </nav>
        }
        @if (timers().length) {
          <aside class="timers">
            @for (t of timers(); track t.endsAt) {
              <div class="timer" [class.done]="remaining(t) === 0">
                <span>{{ t.label }}</span>
                <strong>{{ format(remaining(t)) }}</strong>
                <button mat-icon-button aria-label="Arrêter le minuteur" (click)="stop(t)">
                  <mat-icon>close</mat-icon>
                </button>
              </div>
            }
          </aside>
        }
      </div>
    } @else {
      <mat-progress-bar mode="indeterminate" />
    }
  `,
  styles: `
    .cooking {
      min-height: 100dvh;
      display: flex;
      flex-direction: column;
      padding: 1rem;
      box-sizing: border-box;
      gap: 1rem;
    }
    header {
      display: flex;
      align-items: center;
      gap: 0.5rem;
    }
    h1 {
      flex: 1;
      margin: 0;
      font: var(--mat-sys-title-large);
    }
    .ingredients {
      margin: 0;
      padding: 1rem 1rem 1rem 2rem;
      background: var(--mat-sys-surface-container);
      border-radius: var(--mat-sys-corner-large);
      font: var(--mat-sys-body-large);
    }
    main {
      flex: 1;
      display: flex;
      flex-direction: column;
      justify-content: center;
      gap: 1.5rem;
    }
    .step {
      font: var(--mat-sys-headline-medium);
      margin: 0;
    }
    nav {
      display: flex;
      justify-content: space-between;
      gap: 1rem;
    }
    nav button {
      flex: 1;
      height: 3.5rem;
    }
    .timers {
      display: flex;
      flex-direction: column;
      gap: 0.5rem;
    }
    .timer {
      display: grid;
      grid-template-columns: 1fr auto auto;
      align-items: center;
      gap: 0.5rem;
      padding: 0.5rem 1rem;
      border-radius: var(--mat-sys-corner-medium);
      background: var(--mat-sys-secondary-container);
    }
    .timer span {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .timer strong {
      font: var(--mat-sys-title-large);
      font-variant-numeric: tabular-nums;
    }
    .timer.done {
      background: var(--mat-sys-error-container);
      color: var(--mat-sys-on-error-container);
    }
  `,
})
export class CookingPage {
  private readonly api = inject(RecipesService);

  readonly id = input.required<string>();
  /** Équivalents adulte servis (« 2,6 »), repris de la fiche. */
  readonly portions = input<string>();

  protected readonly current = signal(0);
  protected readonly showIngredients = signal(true);
  protected readonly timers = signal<Timer[]>([]);
  private readonly now = signal(Date.now());
  protected readonly quantityText = quantityText;

  protected readonly recipe = rxResource({
    params: () => ({ id: this.id(), servings: parseQuantity(this.portions() ?? '', 'COUNT') }),
    stream: ({ params }) =>
      this.api.getRecipe({ id: params.id, servings: params.servings || undefined }),
  });

  protected readonly running = computed(() => this.timers().length);

  constructor() {
    const tick = setInterval(() => this.now.set(Date.now()), 1_000);
    let lock: { release(): Promise<void> } | null = null;
    // L'écran reste allumé pendant qu'on cuisine (si le navigateur le permet).
    const wakeLock = (
      navigator as Navigator & {
        wakeLock?: { request(type: 'screen'): Promise<{ release(): Promise<void> }> };
      }
    ).wakeLock;
    void wakeLock
      ?.request('screen')
      .then((l) => (lock = l))
      .catch(() => undefined);
    inject(DestroyRef).onDestroy(() => {
      clearInterval(tick);
      void lock?.release();
    });
  }

  protected startTimer(step: number, text: string, minutes: number): void {
    const label = `Étape ${step + 1} : ${text.slice(0, 40)}`;
    this.timers.update((t) => [...t, { step, label, endsAt: Date.now() + minutes * 60_000 }]);
  }

  protected stop(timer: Timer): void {
    this.timers.update((t) => t.filter((x) => x !== timer));
  }

  protected remaining(timer: Timer): number {
    return Math.max(0, Math.ceil((timer.endsAt - this.now()) / 1_000));
  }

  protected format(seconds: number): string {
    const m = Math.floor(seconds / 60);
    return `${m}:${String(seconds % 60).padStart(2, '0')}`;
  }
}
