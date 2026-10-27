import { Page, expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

/** Confirme l'envoi dans l'aperçu (R-REC-81) : la fenêtre doit se fermer. */
async function confirmSend(page: Page): Promise<void> {
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText("Envoyer à l'IA ?")).toBeVisible();
  await expect(async () => {
    if (await dialog.isVisible())
      await dialog.getByRole('button', { name: 'Envoyer', exact: true }).click();
    await expect(dialog).toBeHidden({ timeout: 2_000 });
  }).toPass();
}

/** Active l'IA factice dans les réglages (ADR-0014). */
async function enableAi(page: Page): Promise<void> {
  await page.goto('/reglages');
  const section = page.locator('section', { hasText: 'Intelligence artificielle' });
  const toggle = section.getByRole('switch');
  // Au tout premier chargement, un clic peut précéder la liaison du formulaire : on vérifie l'état obtenu.
  await expect(async () => {
    if ((await toggle.getAttribute('aria-checked')) !== 'true') await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true', { timeout: 1_000 });
  }).toPass();
  await section.getByLabel('Fournisseur').click();
  await page.getByRole('option', { name: 'OpenAI ou compatible' }).click();
  await section.getByLabel('Modèle').fill('modele-de-test');
  await section.getByLabel("Adresse de l'API").fill('http://127.0.0.1:3197/v1');
  await section.getByLabel("Clé d'accès").fill('cle-de-test');
  await section.getByRole('button', { name: 'Enregistrer' }).click();
  await expect(page.getByText('Réglages de l’IA enregistrés')).toBeVisible();
  // La clé n'est jamais réaffichée.
  await expect(section.getByLabel("Clé d'accès")).toHaveValue('');
  await expect(section.getByRole('button', { name: 'Effacer la clé' })).toBeVisible();
}

test('IA : une recette demandée arrive en brouillon, après l’aperçu de ce qui part (REC-19)', async ({
  page,
}) => {
  await login(page);
  await enableAi(page);
  await page.goto('/recettes/ia');
  await page.getByLabel('Consigne').fill('une soupe de courgettes');
  await page.getByRole('button', { name: 'Proposer une recette' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText("Envoyer à l'IA ?")).toBeVisible();
  await expect(dialog.getByText(/A : 1 équivalent adulte/)).toBeVisible();
  await expect(dialog.getByText(/Alex/)).toHaveCount(0);
  await confirmSend(page);
  await expect(page.getByRole('heading', { name: 'Revue de l’import' })).toBeVisible();
  await expect(page.getByLabel('Nom')).toHaveValue('Velouté de courgettes');
  await page.getByRole('button', { name: 'Abandonner l’import' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Abandonner' }).click();
  await expect(page.getByRole('heading', { name: 'Importer des recettes' })).toBeVisible();
});

test('IA : des menus proposés, acceptés repas par repas (PLA-12, R-PLA-64, R-PLA-66)', async ({
  page,
}) => {
  // Une semaine propre à chaque format.
  const week = isMobile(page) ? '2026-12-07' : '2026-11-30';
  await login(page);
  await enableAi(page);
  await page.goto(`/repas/ia?semaine=${week}`);
  await page.getByRole('button', { name: /Proposer les menus/ }).click();
  await confirmSend(page);
  await expect(page.getByRole('heading', { name: /Menus proposés/ })).toBeVisible();
  await expect(page.getByText('Recettes nouvelles à relire')).toBeVisible();
  await expect(page.getByText('Recette à relire')).toBeVisible();
  await page.getByRole('button', { name: 'Accepter tout ce qui est prêt' }).click();
  await expect(page.getByText(/1 repas ajouté\(s\) au calendrier/)).toBeVisible();
  await expect(page.getByText('Accepté', { exact: true })).toBeVisible();

  // Relire la recette nouvelle, puis accepter son repas.
  await page.getByRole('link', { name: 'Relire' }).click();
  await expect(page.getByLabel('Nom')).toHaveValue('Velouté de courgettes');
  await page.getByRole('button', { name: 'Valider la recette' }).click();
  await expect(page.getByText(/Recette enregistrée/)).toBeVisible();
  await page.goto(`/repas?semaine=${week}`);
  await page.getByRole('link', { name: /Menus proposés par l.IA/ }).click();
  await page.getByRole('button', { name: 'Accepter tout ce qui est prêt' }).click();
  await expect(page.getByText(/1 repas ajouté\(s\) au calendrier/)).toBeVisible();
  await page.goto(`/repas?semaine=${week}`);
  await expect(page.getByRole('link', { name: /Velouté de courgettes/ })).toBeVisible();
});
