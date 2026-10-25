import { expect, test } from '@playwright/test';

import { TODAY, expiredLot, login, openAddLot } from './helpers';

test('connexion : un mauvais mot de passe est refusé, le bon ouvre l’accueil', async ({ page }) => {
  await page.clock.install({ time: new Date(TODAY) });
  await page.goto('/');
  await expect(page).toHaveURL(/connexion/);
  await page.getByLabel('Mot de passe').fill('mauvais');
  await page.getByRole('button', { name: 'Se connecter' }).click();
  await expect(page.getByText('Mot de passe incorrect.')).toBeVisible();
  await login(page);
});

test('accueil : les lots à date dépassée et à vérifier sont en priorité (TDB-01)', async ({
  page,
}) => {
  await login(page);
  const priority = page.locator('section', { hasText: 'À consommer en priorité' });
  await expect(priority.getByText('Emmental râpé')).toBeVisible();
  await expect(priority.getByText(/Bientôt/).first()).toBeVisible();
  await expect(priority.getByText('À vérifier').first()).toBeVisible();
});

test('stock : ajouter, consommer et annuler (STK-02, STK-03, R-STK-11)', async ({ page }) => {
  await login(page);
  await openAddLot(page);
  await page.getByRole('combobox', { name: 'Aliment ou produit' }).fill('courge');
  await page
    .getByRole('option', { name: /Courgette/ })
    .first()
    .click();
  await page.getByLabel('Quantité par lot').fill('600');
  await page.getByLabel('Péremption').click();
  await page.getByRole('option', { name: 'Sans date' }).click();
  await page.getByRole('button', { name: 'Ajouter', exact: true }).click();
  await expect(page.getByText('Courgette ajouté au stock')).toBeVisible();

  await page.goto('/stock');
  await page.getByPlaceholder('Filtrer').fill('courgette');
  await page
    .getByRole('link', { name: /Courgette.*600\sg/ })
    .first()
    .click();
  await expect(page.getByRole('heading', { name: 'Courgette' })).toBeVisible();
  await page.getByRole('button', { name: 'Consommer' }).click();
  await page.getByLabel('Quantité').fill('250');
  await page.getByRole('dialog').getByRole('button', { name: 'Consommer' }).click();
  await expect(page.locator('.quantity')).toHaveText('350 g');

  await page.getByRole('button', { name: 'Annuler ce mouvement' }).first().click();
  await page.getByRole('button', { name: 'Annuler le mouvement' }).click();
  await expect(page.getByText('Mouvement annulé')).toBeVisible();
  await expect(page.locator('.quantity')).toHaveText('600 g');
});

test('stock : une date dépassée se juge encore bonne (R-STK-37)', async ({ page }) => {
  await login(page);
  // Un lot à DLC dépassée, propre à ce test : les deux profils partagent le même serveur.
  const lotId = await expiredLot(page, 'Yaourt nature');
  await page.goto(`/stock/${lotId}`);
  await page.getByRole('button', { name: 'Encore bon' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Encore bon' }).click();
  await expect(page.getByText('Jugé encore bon').first()).toBeVisible();
  await expect(page.getByText(/Bientôt · limite/)).toBeVisible();
});

test('aliments : rechercher sans accents et ouvrir une fiche (ALI-06)', async ({ page }) => {
  await login(page);
  await page.goto('/aliments');
  await page.getByPlaceholder('Nom ou alias').fill('gruyere');
  await page.getByRole('link', { name: /Emmental râpé/ }).click();
  await expect(page.getByRole('heading', { name: 'Emmental râpé' })).toBeVisible();
  await expect(page.getByText('Valeurs nutritionnelles')).toBeVisible();
});

test('inventaire : constater une quantité (STK-05)', async ({ page }) => {
  await login(page);
  await page.goto('/stock/inventaire');
  await page.getByRole('option', { name: 'Placard' }).click();
  const rice = page.locator('.row', { hasText: 'Riz basmati' });
  await rice.getByLabel('Constaté').fill('800 g');
  await page.getByRole('button', { name: 'Enregistrer l’inventaire' }).click();
  await expect(page.getByText('Inventaire enregistré')).toBeVisible();
});
