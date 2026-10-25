import { IngredientView } from '../../api/models/ingredient-view';
import { formatHalves } from '../../core/quantity';

/** Quantité affichée : saisie mise à l'échelle, demi-pièces pour un aliment compté (R-REC-14). */
export function quantityText(i: IngredientView): string {
  if (i.to_taste) {
    return '';
  }
  if (i.display_amount != null && !i.unit_id && !i.unit_label) {
    return formatHalves(i.display_amount);
  }
  return [i.quantity, i.unit_label].filter(Boolean).join(' ');
}
