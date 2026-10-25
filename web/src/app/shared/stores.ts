import { Injectable, computed, inject } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';

import { FoodsService } from '../api/services/foods.service';
import { StockService } from '../api/services/stock.service';
import { DataVersion } from '../core/data-version';

/** Emplacements, partagés par tous les écrans et rechargés à chaque modification. */
@Injectable({ providedIn: 'root' })
export class LocationsStore {
  private readonly api = inject(StockService);
  private readonly version = inject(DataVersion);

  readonly resource = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listLocations(),
  });

  readonly list = computed(() => (this.resource.hasValue() ? this.resource.value() : []));
}

/** Familles d'aliments, à plat, triées par chemin (« Produits laitiers › Laits »). */
@Injectable({ providedIn: 'root' })
export class GroupsStore {
  private readonly api = inject(FoodsService);
  private readonly version = inject(DataVersion);

  readonly resource = rxResource({
    params: () => this.version.value(),
    stream: () => this.api.listFoodGroups(),
  });

  readonly list = computed(() => (this.resource.hasValue() ? this.resource.value() : []));
}
