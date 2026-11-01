import { expect, test } from '@playwright/test';

import { login } from './helpers';

// Caméra factice de Chromium, autorisée : aucun matériel ni réseau.
test.use({
  permissions: ['camera'],
  launchOptions: {
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'],
  },
});

test('mode caméra : elle démarre dès l’ouverture du scan, sans second clic (R-STK-50)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/reglages');
  await page.getByRole('radio', { name: /Caméra/ }).check();
  await page.goto('/stock/scan');
  await expect(page.locator('video')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Arrêter la caméra' })).toBeVisible();
});

test('mode lecteur : pas de caméra, le lecteur est prêt (R-STK-50)', async ({ page }) => {
  await login(page);
  await page.goto('/reglages');
  await page.getByRole('radio', { name: /Lecteur de code-barres/ }).check();
  await page.goto('/stock/scan');
  await expect(page.getByText('Lecteur de code-barres prêt')).toBeVisible();
  await expect(page.locator('video')).toHaveCount(0);
});
