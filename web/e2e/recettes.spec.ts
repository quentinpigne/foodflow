import { expect, test } from '@playwright/test';

import { isMobile, login } from './helpers';

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

const PAGES = 'http://127.0.0.1:3199/e2e-pages';

test('import par lot : une par une, erreurs à part, revue enchaînée (REC-17, R-REC-69)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/recettes/import');
  await page
    .getByLabel('Adresses des recettes')
    .fill(
      `${PAGES}/gratin-graph.html\n${PAGES}/absente.html\n${PAGES}/crepes-simple.html\n${PAGES}/gratin-graph.html`,
    );
  await expect(page.getByText('3 adresse(s)')).toBeVisible();
  await page.getByRole('button', { name: 'Importer', exact: true }).click();
  await expect(page.getByText('Lot en cours · 3 / 3')).toBeVisible({ timeout: 20_000 });
  await expect(page.getByText(/ne publie pas de recette/)).toBeVisible();
  // L'adresse en échec reste dans le champ, pour réessayer.
  await expect(page.getByLabel('Adresses des recettes')).toHaveValue(`${PAGES}/absente.html`);

  // Les brouillons se relisent à la suite, dans l'ordre du lot.
  await page.getByRole('link', { name: 'Relire' }).click();
  await expect(page.getByLabel('Nom')).toHaveValue('Gratin de courgettes');
  await page.getByRole('button', { name: 'Abandonner l’import' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Abandonner' }).click();
  await expect(page.getByLabel('Nom')).toHaveValue("Pâte à crêpes de l'Ouest");
  await page.getByRole('button', { name: 'Abandonner l’import' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Abandonner' }).click();
  await expect(page.getByRole('heading', { name: 'Importer des recettes' })).toBeVisible();
});

test('envoi depuis le navigateur : favori ou partage (REC-18, R-REC-72)', async ({ page }) => {
  await login(page);
  if (isMobile(page)) {
    // Partage vers la PWA : l'adresse arrive dans le texte partagé.
    const text = `Regarde ${PAGES}/crepes-simple.html`;
    await page.goto(`/recettes/import?title=Cr%C3%AApes&text=${encodeURIComponent(text)}`);
  } else {
    await page.goto('/recettes/import');
    const href = await page.getByRole('link', { name: 'Envoyer à Foodflow' }).getAttribute('href');
    expect(href).toMatch(/^javascript:/);
    // Le favori, cliqué sur la page d'une recette, ouvre Foodflow dans un nouvel onglet.
    await page.goto(`${PAGES}/crepes-simple.html`);
    const opened = page.context().waitForEvent('page');
    await page.evaluate((code) => new Function(code)(), decodeURIComponent(href!.slice(11)));
    page = await opened;
  }
  await expect(page.getByRole('heading', { name: 'Revue de l’import' })).toBeVisible();
  await expect(page.getByLabel('Nom')).toHaveValue("Pâte à crêpes de l'Ouest");
  await page.getByRole('button', { name: 'Abandonner l’import' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Abandonner' }).click();
  await expect(page.getByRole('heading', { name: 'Importer des recettes' })).toBeVisible();
});

test('revue : créer l’aliment d’un ingrédient inconnu, retenu à l’import suivant (R-REC-66)', async ({
  page,
}, testInfo) => {
  const ingredient = `Os à moelle ${testInfo.project.name}`;
  const jsonLd = (name: string) =>
    JSON.stringify({
      '@type': 'Recipe',
      name,
      recipeIngredient: [`2 ${ingredient}`, '4 tomates'],
      recipeInstructions: 'Tout mijoter.',
    });
  await login(page);
  await page.goto('/recettes/import');
  await page.locator('input[type=file]').setInputFiles({
    name: 'pot-au-feu.jsonld',
    mimeType: 'application/ld+json',
    buffer: Buffer.from(jsonLd(`Pot-au-feu (${testInfo.project.name})`)),
  });
  await expect(page.getByRole('heading', { name: 'Revue de l’import' })).toBeVisible();
  // La ligne sans aliment est en tête : on crée l'aliment depuis le choix de l'ingrédient, une fois la revue
  // chargée.
  await expect(page.getByRole('combobox', { name: 'Ingrédient' }).nth(1)).toHaveValue('Tomate');
  const picker = page.getByRole('combobox', { name: 'Ingrédient' }).first();
  await picker.fill('');
  await picker.pressSequentially(ingredient);
  await page.getByRole('option', { name: `Créer l'aliment « ${ingredient} »` }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByLabel('Nom')).toHaveValue(ingredient);
  await dialog.getByRole('combobox', { name: 'Famille' }).click();
  await page.getByRole('option').first().click();
  await dialog.getByRole('button', { name: 'Créer' }).click();
  await expect(picker).toHaveValue(ingredient);
  await page.getByRole('button', { name: 'Valider la recette' }).click();
  await expect(page.getByRole('heading', { name: /Pot-au-feu/ })).toBeVisible();

  // Le rattachement est mémorisé : le même ingrédient est reconnu à l'import suivant.
  await page.goto('/recettes/import');
  await page.locator('input[type=file]').setInputFiles({
    name: 'pot-au-feu-2.jsonld',
    mimeType: 'application/ld+json',
    buffer: Buffer.from(jsonLd(`Pot-au-feu bis (${testInfo.project.name})`)),
  });
  await expect(page.getByRole('combobox', { name: 'Ingrédient' }).first()).not.toHaveValue('');
  await expect(page.getByText('À choisir')).toHaveCount(0);
  await page.getByRole('button', { name: 'Abandonner l’import' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Abandonner' }).click();
  await expect(page.getByRole('heading', { name: 'Importer des recettes' })).toBeVisible();
});

test('choix d’un ingrédient : un aliment existant est proposé, jamais recréé (R-ALI-10)', async ({
  page,
}) => {
  await login(page);
  await page.goto('/recettes/nouvelle');
  const picker = page.getByRole('combobox', { name: 'Ingrédient' }).first();
  await picker.pressSequentially('tomate');
  await expect(page.getByRole('option', { name: 'Tomate', exact: true })).toBeVisible();
  // L'aliment existe : pas de « Créer l'aliment » pour ce nom.
  await expect(page.getByRole('option', { name: /Créer l'aliment/ })).toHaveCount(0);

  // Un nom saisi autrement, puis corrigé en un nom pris : la fenêtre propose l'aliment existant.
  await picker.fill('');
  await picker.pressSequentially('tomat');
  await page.getByRole('option', { name: "Créer l'aliment « tomat »" }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nom').fill('Tomate');
  await dialog.getByRole('combobox', { name: 'Famille' }).click();
  await page.getByRole('option').first().click();
  await dialog.getByRole('button', { name: 'Créer' }).click();
  await dialog.getByRole('button', { name: "Utiliser l'aliment existant « Tomate »" }).click();
  await expect(picker).toHaveValue('Tomate');
});
