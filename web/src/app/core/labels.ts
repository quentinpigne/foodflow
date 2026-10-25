/** Libellés français des valeurs d'énumération de l'API (glossaire). */

import { ExpiryKind } from '../api/models/expiry-kind';
import { ExpiryStatus } from '../api/models/expiry-status';
import { LocationKind } from '../api/models/location-kind';
import { Measure } from '../api/models/measure';
import { MovementKind } from '../api/models/movement-kind';
import { PresenceState } from '../api/models/presence-state';
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
