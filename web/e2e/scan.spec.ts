import { expect, test } from '@playwright/test';

import { login } from './helpers';

/** Code-barres du lait Lactel du jeu de démonstration : un produit connu, sans appel à Open Food Facts. */
const LACTEL = '2000000000015';

test('ranger par scan : deux lectures, une validation, une annulation (STK-12, R-STK-51, R-STK-54)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/stock/scan');
  const code = page.getByLabel('Code-barres');
  for (let i = 0; i < 2; i++) {
    await code.fill(LACTEL);
    await code.press('Enter');
  }
  await expect(page.getByText('Lait demi-écrémé 1 L × 2')).toBeVisible();
  await expect(page.getByText('× 2', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: /Mettre en stock \(2 lots\)/ }).click();
  await expect(page.getByText('2 lots ajoutés au stock')).toBeVisible();
  await page.getByRole('button', { name: 'Annuler' }).click();
  await expect(page.getByText('Mise en stock annulée')).toBeVisible();
});

test('ranger par scan : un code inconnu est à rattacher, un code faux refusé (R-STK-50, R-STK-52)', async ({
  page,
}) => {
  await login(page);
  // Aucun test n'appelle le réseau : Open Food Facts est coupé pour ce code inconnu.
  await page.request.put('/api/v1/settings/off', { data: { enabled: false } });
  await page.goto('/stock/scan');
  const code = page.getByLabel('Code-barres');
  await code.fill('3017620422004');
  await code.press('Enter');
  await expect(page.getByText('Code 3017620422004 illisible ou erroné')).toBeVisible();
  await code.fill('96385074');
  await code.press('Enter');
  await expect(page.getByText('Code inconnu')).toBeVisible();
  await expect(page.getByRole('button', { name: /Rattacher à un aliment/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Mettre en stock/ })).toBeDisabled();
});

test('inventaire par scan : le lot lu est constaté (STK-13, R-STK-55)', async ({ page }) => {
  await login(page);
  await page.goto('/stock/inventaire/scan');
  await page.getByRole('option', { name: 'Frigo' }).click();
  const code = page.getByLabel('Code-barres');
  await code.fill(LACTEL);
  await code.press('Enter');
  await expect(page.getByText(/Lait demi-écrémé 1 L (constaté|trouvé en plus)/)).toBeVisible();
  await page.getByRole('button', { name: 'Enregistrer l’inventaire' }).click();
  await expect(page.getByText('Inventaire enregistré')).toBeVisible();
});

test('ranger par scan : la ligne s’affiche dès la lecture, avant la réponse (R-STK-50)', async ({
  page,
}) => {
  await login(page);
  // Une recherche lente, comme quand Open Food Facts tarde à répondre.
  let release: () => void = () => undefined;
  const answered = new Promise<void>((resolve) => (release = resolve));
  await page.route('**/api/v1/barcodes/*', async (route) => {
    await answered;
    await route.continue();
  });
  await page.goto('/stock/scan');
  const code = page.getByLabel('Code-barres');
  await code.fill(LACTEL);
  await code.press('Enter');
  await expect(page.getByText('Recherche…', { exact: true })).toBeVisible();
  await code.fill(LACTEL);
  await code.press('Enter');
  await expect(page.getByText('× 2', { exact: true })).toBeVisible();
  release();
  await expect(page.getByText('Lait demi-écrémé 1 L × 2')).toBeVisible();
  await expect(page.getByText('Recherche…', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Tout effacer' }).click();
});
