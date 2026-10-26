import { Page, expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

/** La carte du dîner de samedi : le gratin, pas ses restes. */
function gratinDinner(page: Page) {
  return page.getByRole('link', { name: /Gratin de courgettes/ }).filter({ hasNotText: 'Restes' });
}

test('calendrier : les repas de la semaine et leurs signalements (PLA-03, R-PLA-50)', async ({
  page,
}) => {
  await login(page);
  // L'accueil signale le dîner d'hier, pas encore validé (R-PLA-35).
  await expect(page.getByRole('link', { name: '1 repas passé à valider' })).toBeVisible();
  await page.getByRole('link', { name: '1 repas passé à valider' }).click();
  await expect(page.getByRole('heading', { name: 'Repas' })).toBeVisible();
  // Le gratin de samedi, cuisiné pour plus ; ses restes ce midi.
  await expect(gratinDinner(page)).toBeVisible();
  await expect(page.getByRole('link', { name: /Restes : Gratin de courgettes/ })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Valider le repas passé' })).toBeVisible();
  // Semaine suivante : les pâtes de lundi et les crêpes de mardi, trop longues pour un soir de semaine.
  await page.getByRole('button', { name: 'Semaine suivante' }).click();
  await expect(page.getByRole('link', { name: /Pâtes à la sauce tomate/ })).toBeVisible();
  await page.getByRole('link', { name: /Crêpes/ }).click();
  await expect(page.getByText(/Pas prêt à l’heure/)).toBeVisible();
});

test('repas : créer, composer, enregistrer (PLA-03, R-PLA-5)', async ({ page }, testInfo) => {
  // Une semaine propre à chaque format, loin de celles de la démonstration.
  const day = isMobile(page) ? '2026-11-17' : '2026-11-10';
  await login(page);
  await page.goto(`/repas?semaine=${day}`);
  if (isMobile(page)) {
    await page.getByRole('button', { name: 'Dîner' }).first().click();
  } else {
    await page.getByRole('button', { name: /Ajouter : Dîner, Mardi/ }).click();
  }
  await expect(page.getByRole('heading', { name: /^Dîner · / })).toBeVisible();
  await expect(page.getByText('2,6 éq. adulte')).toBeVisible();
  await page.getByLabel('Ajouter une recette').pressSequentially('gratin');
  await page.getByRole('option', { name: 'Gratin de courgettes' }).click();
  await expect(page.getByLabel('Portions cuisinées')).toHaveValue('2,6');
  await page.getByLabel('Portions cuisinées').fill('4');
  await page.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Repas enregistré')).toBeVisible();
  await expect(page.getByText(/Reste prévu : 1,4 portion/)).toBeVisible();
  await expect(page.getByText(/Reste de 1,4 portion\(s\) à placer/)).toBeVisible();
  void testInfo;
  // Ménage : le repas est supprimé.
  await page.getByRole('button', { name: 'Supprimer le repas' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Supprimer' }).click();
  await expect(page.getByRole('heading', { name: 'Repas' })).toBeVisible();
});

test('validation : sorties, restes, puis annulation (PLA-04, R-PLA-34)', async ({ page }) => {
  await login(page);
  await page.goto('/repas');
  await gratinDinner(page).click();
  await page.getByRole('button', { name: 'Valider le repas' }).click();
  await expect(page.getByRole('heading', { name: 'Valider le repas' })).toBeVisible();
  await expect(page.getByText('Courgette', { exact: true }).first()).toBeVisible();
  await expect(page.getByLabel('Portions restantes')).toHaveValue('3,4');
  await page.getByLabel('Portions restantes').fill('2');
  await page.getByRole('button', { name: 'Valider', exact: true }).click();
  await expect(page.getByText('Repas validé')).toBeVisible();
  await expect(page.getByText(/mangé aux 67 %/)).toBeVisible();

  // Les restes sont en stock, et le déjeuner de dimanche les mange.
  await page.goto('/stock');
  await expect(page.getByRole('link', { name: /Restes : Gratin de courgettes/ })).toBeVisible();

  // Annulation : le repas redevient prévu, les restes disparaissent.
  await page.goto('/repas');
  await gratinDinner(page).click();
  await page.getByRole('button', { name: 'Annuler la validation' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Annuler la validation' }).click();
  await expect(page.getByText('Validation annulée')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Valider le repas' })).toBeVisible();
  await page.goto('/stock');
  await expect(page.getByRole('link', { name: /Restes : Gratin de courgettes/ })).toHaveCount(0);
});

test('foyer : membres et semaine type (PLA-01, PLA-02)', async ({ page }) => {
  await login(page);
  await page.goto('/foyer');
  await expect(page.getByText('Lou')).toBeVisible();
  await expect(page.getByText('0,6 éq. adulte')).toBeVisible();
  await page.getByRole('tab', { name: 'Semaine type' }).click();
  await expect(page.getByRole('heading', { name: 'Dîner' })).toBeVisible();
  await page.getByRole('tab', { name: 'Repas habituels' }).click();
  await expect(page.getByLabel('Nom').first()).toHaveValue('Bol de lait');
});

test('bilan : prévu et réel, critère par critère (NUT-04)', async ({ page }) => {
  await login(page);
  await page.goto('/bilan');
  await expect(page.getByText('Énergie, par jour')).toBeVisible();
  await expect(page.getByText(/créneau\(x\) sans repas/)).toBeVisible();
  await page.getByRole('radio', { name: 'Réel' }).click();
  await expect(page.getByText(/D’après les repas validés/)).toBeVisible();
});
