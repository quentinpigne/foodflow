import { Routes } from '@angular/router';

import { requireSession } from './core/auth';

export const routes: Routes = [
  {
    path: 'recettes/:id/cuisine',
    title: 'En cuisine — Foodflow',
    canActivate: [requireSession],
    loadComponent: () => import('./features/recipes/cooking').then((m) => m.CookingPage),
  },
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
        path: 'recettes',
        title: 'Recettes — Foodflow',
        loadComponent: () => import('./features/recipes/recipes').then((m) => m.RecipesPage),
      },
      {
        path: 'recettes/nouvelle',
        title: 'Nouvelle recette — Foodflow',
        loadComponent: () => import('./features/recipes/recipe-editor').then((m) => m.RecipeEditor),
      },
      {
        path: 'recettes/import',
        title: 'Importer une recette — Foodflow',
        loadComponent: () => import('./features/recipes/import').then((m) => m.RecipeImportPage),
      },
      {
        path: 'recettes/import/:draftId',
        title: 'Brouillon — Foodflow',
        loadComponent: () => import('./features/recipes/recipe-editor').then((m) => m.RecipeEditor),
      },
      {
        path: 'recettes/:id',
        title: 'Recette — Foodflow',
        loadComponent: () => import('./features/recipes/recipe-page').then((m) => m.RecipePage),
      },
      {
        path: 'recettes/:id/modifier',
        title: 'Modifier la recette — Foodflow',
        loadComponent: () => import('./features/recipes/recipe-editor').then((m) => m.RecipeEditor),
      },
      {
        path: 'objectifs',
        title: 'Objectifs — Foodflow',
        loadComponent: () => import('./features/goals/goals').then((m) => m.GoalsPage),
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
