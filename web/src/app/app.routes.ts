import { Routes } from '@angular/router';

import { requireSession } from './core/auth';

export const routes: Routes = [
  {
    path: 'connexion',
    title: 'Connexion — Foodflow',
    loadComponent: () => import('./features/login/login').then((m) => m.LoginPage),
  },
  {
    path: '',
    canActivate: [requireSession],
    loadComponent: () => import('./features/shell/shell').then((m) => m.Shell),
    children: [
      {
        path: '',
        title: 'Accueil — Foodflow',
        loadComponent: () => import('./features/home/home').then((m) => m.HomePage),
      },
      {
        path: 'stock',
        title: 'Stock — Foodflow',
        loadComponent: () => import('./features/stock/stock').then((m) => m.StockPage),
      },
      {
        path: 'stock/inventaire',
        title: 'Inventaire — Foodflow',
        loadComponent: () => import('./features/stock/inventory').then((m) => m.InventoryPage),
      },
      {
        path: 'stock/:id',
        title: 'Lot — Foodflow',
        loadComponent: () => import('./features/stock/lot-page').then((m) => m.LotPage),
      },
      {
        path: 'aliments',
        title: 'Aliments — Foodflow',
        loadComponent: () => import('./features/foods/foods').then((m) => m.FoodsPage),
      },
      {
        path: 'aliments/:id',
        title: 'Aliment — Foodflow',
        loadComponent: () => import('./features/foods/food-page').then((m) => m.FoodPage),
      },
      {
        path: 'reglages',
        title: 'Réglages — Foodflow',
        loadComponent: () => import('./features/settings/settings').then((m) => m.SettingsPage),
      },
    ],
  },
  { path: '**', redirectTo: '' },
];
