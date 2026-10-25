import { Pipe, PipeTransform } from '@angular/core';

import { Measure } from '../api/models/measure';
import { formatQuantity } from '../core/quantity';

/** `{{ raw | quantity: 'MASS' }}` → « 1,5 kg ». */
@Pipe({ name: 'quantity' })
export class QuantityPipe implements PipeTransform {
  transform(raw: number | null | undefined, measure: Measure): string {
    return formatQuantity(raw, measure);
  }
}
