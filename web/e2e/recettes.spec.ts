import { expect, test } from '@playwright/test';

import { login } from './helpers';

test('recettes : chercher, ouvrir et mettre à l’échelle (REC-02, REC-09)', async ({ page }) => {
  await login(page);
  await page.goto('/recettes');
  await expect(page.getByRole('link', { name: /Gratin de courgettes/ })).toBeVisible();
  await page.getByPlaceholder('Nom, étiquette, ingrédient').fill('chandeleur');
  await expect(page.getByRole('link', { name: /Gratin de courgettes/ })).toHaveCount(0);
  await page.getByRole('link', { name: /Crêpes/ }).click();
  await expect(page.getByRole('heading', { name: 'Crêpes' })).toBeVisible();

  await page.getByLabel('Pour (équivalents adulte)').fill('3');
  await expect(page.getByText('125 g')).toBeVisible();
  await expect(page.getByText('25 cL')).toBeVisible();
});

test('mode cuisine : étapes une à une et minuteur (REC-08)', async ({ page }) => {
  await login(page);
  await page.goto('/recettes');
  await page.getByRole('link', { name: /Crêpes/ }).click();
  await page.getByRole('link', { name: 'Cuisiner' }).click();
  await expect(page.getByText('Étape 1 / 3')).toBeVisible();
  await page.getByRole('button', { name: 'Suivante' }).click();
  await expect(page.getByText('Laisser reposer une heure.')).toBeVisible();
  await page.getByRole('button', { name: /Minuteur 60 min/ }).click();
  await expect(page.getByText('60:00')).toBeVisible();
  await page.getByRole('link', { name: 'Quitter le mode cuisine' }).click();
  await expect(page.getByRole('heading', { name: 'Crêpes' })).toBeVisible();
});

test('import : un JSON-LD devient un brouillon, puis une recette (REC-07, REC-11)', async ({
  page,
}, testInfo) => {
  const name = `Soupe de test (${testInfo.project.name})`;
  const jsonLd = JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'Recipe',
    name,
    recipeYield: '4',
    prepTime: 'PT15M',
    cookTime: 'PT30M',
    recipeIngredient: ['4 tomates', '1 œuf', 'Sel, poivre'],
    recipeInstructions: [{ '@type': 'HowToStep', text: 'Tout cuire ensemble.' }],
  });
  await login(page);
  await page.goto('/recettes/import');
  await page.locator('input[type=file]').setInputFiles({
    name: 'soupe.jsonld',
    mimeType: 'application/ld+json',
    buffer: Buffer.from(jsonLd),
  });
  await expect(page.getByRole('heading', { name: 'Revue de l’import' })).toBeVisible();
  await expect(page.getByLabel('Nom')).toHaveValue(name);
  // « Sel, poivre » : le sel n'a pas de cible sûre, il bloque la validation ; on le retire.
  await page.getByRole('button', { name: "Retirer l'ingrédient" }).first().click();
  await page.getByRole('button', { name: 'Valider la recette' }).click();
  await expect(page.getByRole('heading', { name })).toBeVisible();
  await expect(page.getByText('Tout cuire ensemble.')).toBeVisible();
});

test('objectifs : la version en vigueur et les créneaux de repas (NUT-01, NUT-03)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/objectifs');
  await expect(page.getByRole('heading', { name: 'Objectifs nutritionnels' })).toBeVisible();
  await expect(page.getByText('en vigueur', { exact: true })).toBeVisible();
  await expect(page.getByText('18 critères')).toBeVisible();
  await expect(page.getByText('En vigueur : non modifiable')).toBeVisible();
  await expect(page.getByText('Total : 100 %')).toBeVisible();
});
