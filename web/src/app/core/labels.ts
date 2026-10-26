/** Libellés français des valeurs d'énumération de l'API (glossaire). */

import { Confidence } from '../api/models/confidence';
import { CriterionKind } from '../api/models/criterion-kind';
import { ExpiryKind } from '../api/models/expiry-kind';
import { ExpiryStatus } from '../api/models/expiry-status';
import { Importance } from '../api/models/importance';
import { LocationKind } from '../api/models/location-kind';
import { MealStatus } from '../api/models/meal-status';
import { Measure } from '../api/models/measure';
import { MovementKind } from '../api/models/movement-kind';
import { NutritionGroup } from '../api/models/nutrition-group';
import { Period } from '../api/models/period';
import { PreferenceLevel } from '../api/models/preference-level';
import { PresenceState } from '../api/models/presence-state';
import { RecipeKind } from '../api/models/recipe-kind';
import { Signal } from '../api/models/signal';
import { Status } from '../api/models/status';
import { TrackingMode } from '../api/models/tracking-mode';
import { ValueSource } from '../api/models/value-source';
import { WasteReason } from '../api/models/waste-reason';

export const MEASURE_LABELS: Record<Measure, string> = {
  MASS: 'Masse (g, kg)',
  VOLUME: 'Volume (mL, L)',
  COUNT: 'Nombre (pièces)',
};

export const TRACKING_LABELS: Record<TrackingMode, string> = {
  QUANTITY: 'Quantité exacte',
  PRESENCE: 'Présence seulement',
};

export const EXPIRY_KIND_LABELS: Record<ExpiryKind, string> = {
  USE_BY: 'À consommer jusqu’au (DLC)',
  BEST_BEFORE: 'De préférence avant (DDM)',
  NONE: 'Sans date',
};

export const EXPIRY_KIND_SHORT: Record<ExpiryKind, string> = {
  USE_BY: 'DLC',
  BEST_BEFORE: 'DDM',
  NONE: '',
};

export const STATUS_LABELS: Record<ExpiryStatus, string> = {
  OK: 'OK',
  SOON: 'Bientôt',
  EXPIRED: 'Date dépassée',
  PAST_BEST_BEFORE: 'DDM dépassée',
};

export const LOCATION_KIND_LABELS: Record<LocationKind, string> = {
  PANTRY: 'Placard',
  FRIDGE: 'Frigo',
  FREEZER: 'Congélateur',
};

export const LOCATION_ICONS: Record<LocationKind, string> = {
  PANTRY: 'kitchen',
  FRIDGE: 'kitchen',
  FREEZER: 'ac_unit',
};

export const PRESENCE_LABELS: Record<PresenceState, string> = {
  AVAILABLE: 'Disponible',
  LOW: 'Bientôt fini',
  OUT: 'Épuisé',
};

export const MOVEMENT_LABELS: Record<MovementKind, string> = {
  PURCHASE: 'Entrée',
  COOKED: 'Consommé',
  LEFTOVER: 'Reste',
  PREPARED: 'Préparé',
  WASTE: 'Jeté',
  MOVE: 'Déplacé',
  ADJUSTMENT: 'Correction',
  INVENTORY: 'Inventaire',
  OPENED: 'Ouvert',
  CHECKED: 'Jugé encore bon',
};

export const WASTE_LABELS: Record<WasteReason, string> = {
  EXPIRED: 'Périmé',
  SPOILED: 'Abîmé',
  OTHER: 'Autre',
};

export const SOURCE_LABELS: Record<ValueSource, string> = {
  CIQUAL: 'Ciqual',
  OFF: 'Open Food Facts',
  MANUAL: 'Saisie',
  COMPUTED: 'Calculée',
};

export const RECIPE_KIND_LABELS: Record<RecipeKind, string> = {
  MAIN: 'Plat',
  SIDE: 'Accompagnement',
  STARTER: 'Entrée',
  DESSERT: 'Dessert',
  BREAKFAST: 'Petit-déjeuner',
  SNACK: 'Goûter',
  BASE: 'Préparation de base',
};

export const RECIPE_KINDS: RecipeKind[] = [
  'MAIN',
  'SIDE',
  'STARTER',
  'DESSERT',
  'BREAKFAST',
  'SNACK',
  'BASE',
];

export const NUTRITION_GROUP_LABELS: Record<NutritionGroup, string> = {
  VEGETABLES: 'Légumes',
  FRUITS: 'Fruits',
  LEGUMES: 'Légumineuses',
  WHOLE_GRAINS: 'Féculents complets',
  REFINED_STARCHES: 'Féculents',
  DAIRY: 'Produits laitiers',
  POULTRY: 'Volaille',
  RED_MEAT: 'Viande rouge',
  PROCESSED_MEAT: 'Charcuterie',
  FISH: 'Poisson',
  FATTY_FISH: 'Poisson gras',
  EGGS: 'Œufs',
  NUTS: 'Fruits à coque',
  ADDED_FATS: 'Matières grasses ajoutées',
  SWEET_PRODUCTS: 'Produits sucrés',
  SALTY_SNACKS: 'Produits salés à grignoter',
  SUGARY_DRINKS: 'Boissons sucrées',
  OTHER: 'Autres',
};

export const NUTRITION_GROUPS = Object.keys(NUTRITION_GROUP_LABELS) as NutritionGroup[];

export const CRITERION_KIND_LABELS: Record<CriterionKind, string> = {
  ENERGY: 'Énergie',
  NUTRIENT: 'Nutriment',
  GROUP_PORTIONS: 'Portions d’un groupe',
  GROUP_AMOUNT: 'Quantité d’un groupe',
  GROUP_FREQUENCY: 'Fréquence d’un groupe',
  RECIPE_REPEAT: 'Même recette',
  FAMILY_REPEAT: 'Même famille',
};

export const IMPORTANCE_LABELS: Record<Importance, string> = {
  REQUIRED: 'Obligatoire',
  PREFERRED: 'Souhaité',
  INFO: 'Pour information',
};

export const PERIOD_LABELS: Record<Period, string> = { DAY: 'par jour', WEEK: 'par semaine' };

export const CONFIDENCE_LABELS: Record<Confidence, string> = {
  HIGH: 'Sûr',
  MEDIUM: 'Probable',
  LOW: 'Incertain',
  NONE: 'À choisir',
};

export const MEAL_STATUS_LABELS: Record<MealStatus, string> = {
  PLANNED: 'Prévu',
  DONE: 'Validé',
  SKIPPED: 'Sauté',
  OUT_OF_HOME: 'Hors foyer',
};

export const PREFERENCE_LABELS: Record<PreferenceLevel, string> = {
  LOVES: 'Adore',
  LIKES: 'Aime',
  DISLIKES: 'N’aime pas',
  REFUSES: 'Refuse',
};

/** Jours de la semaine, du lundi (1) au dimanche (7). */
export const WEEKDAYS = ['Lundi', 'Mardi', 'Mercredi', 'Jeudi', 'Vendredi', 'Samedi', 'Dimanche'];

export const ASSESSMENT_LABELS: Record<Status, string> = {
  MET: 'Atteint',
  BELOW: 'En dessous',
  ABOVE: 'Au-dessus',
  INCOMPLETE: 'Incomplet',
  NOT_APPLICABLE: 'Sans repas',
};

/** Texte d'un signalement de repas (R-PLA-50) ; `portions` met en forme des millièmes de portion. */
export function signalText(
  signal: Signal,
  portions: (raw: number) => string,
  memberName: (id: string) => string,
): string {
  switch (signal.kind) {
    case 'TIME_EXCEEDED':
      return `Temps actif dépassé : ${formatMinutes(signal.needed)} pour ${formatMinutes(signal.available)}`;
    case 'READY_BY_EXCEEDED':
      return `Pas prêt à l’heure : ${formatMinutes(signal.needed)} pour ${formatMinutes(signal.available)}`;
    case 'REFUSED':
      return `${memberName(signal.member)} refuse ${signal.item}`;
    case 'REQUIRED_ROLE_EMPTY':
      return `${signal.role} à choisir`;
    case 'LEFTOVER_UNASSIGNED':
      return `Reste de ${portions(signal.portions)} portion(s) à placer`;
    case 'LEFTOVER_PAST_DEADLINE':
      return `Reste mangé après sa date limite (${signal.deadline.split('-').reverse().join('/')})`;
    case 'TO_VALIDATE':
      return 'À valider';
    case 'NO_DINER':
      return 'Aucun convive';
  }
}

/** « 1 h 05 », « 45 min ». */
export function formatMinutes(minutes: number | null | undefined): string {
  if (!minutes) {
    return '0 min';
  }
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h ? `${h} h${m ? ' ' + String(m).padStart(2, '0') : ''}` : `${m} min`;
}
