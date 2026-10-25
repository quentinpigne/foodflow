import { Page, expect } from '@playwright/test';

export const PASSWORD = 'demo-foodflow';
/** Date du jour du serveur de test (e2e/serve.sh) : le navigateur vit à la même date. */
export const TODAY = '2026-10-25T10:00:00';

export async function login(page: Page): Promise<void> {
  await page.clock.install({ time: new Date(TODAY) });
  await page.goto('/connexion');
  await page.getByLabel('Mot de passe').fill(PASSWORD);
  await page.getByRole('button', { name: 'Se connecter' }).click();
  await expect(page.getByRole('heading', { name: 'À consommer en priorité' })).toBeVisible();
}

export function isMobile(page: Page): boolean {
  return (page.viewportSize()?.width ?? 1280) < 600;
}

/** Ouvre « Ajouter au stock » depuis le bouton principal (menu latéral ou « + »). */
export async function openAddLot(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'Ajouter au stock' }).first().click();
  await expect(page.getByRole('heading', { name: 'Ajouter au stock' })).toBeVisible();
}

/** Crée par l'API un lot de l'aliment, à DLC dépassée d'un jour, et renvoie son identifiant. */
export async function expiredLot(page: Page, food: string): Promise<string> {
  const foods = await (
    await page.request.get(`/api/v1/foods?q=${encodeURIComponent(food)}`)
  ).json();
  const locations = await (await page.request.get('/api/v1/locations')).json();
  const response = await page.request.post('/api/v1/lots', {
    data: {
      food_id: foods[0].id,
      location_id: locations.find((l: { kind: string }) => l.kind === 'FRIDGE').id,
      date: '2026-10-20',
      amount: 125000,
      expiry_kind: 'USE_BY',
      expiry_date: '2026-10-24',
    },
  });
  expect(response.ok()).toBeTruthy();
  return (await response.json()).ids[0];
}
