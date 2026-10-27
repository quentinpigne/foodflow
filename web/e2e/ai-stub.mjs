// IA factice des tests de bout en bout : une API de chat compatible OpenAI, en local, aux réponses fixes.
// Elle lit dans la demande les identifiants des créneaux et des recettes : aucun appel réseau réel.
import { createServer } from 'node:http';

const soup = {
  name: 'Velouté de courgettes',
  kind: 'MAIN',
  description: 'Proposé par l’IA de test.',
  portions: 4,
  active_minutes: 15,
  passive_minutes: 20,
  ingredients: ['800 g de courgettes', '50 cl de lait'],
  steps: ['Cuire les courgettes.', 'Mixer avec le lait.'],
  tags: ['soupe'],
};

function plan(text) {
  const slots = [
    ...text.matchAll(/^- (\d{4}-\d{2}-\d{2}) \([^)]*\) \[créneau ([0-9a-f-]{36})\]/gm),
  ];
  const recipe = /\[recette ([0-9a-f-]{36})\] Gratin de courgettes/.exec(text)?.[1];
  const meals = slots.slice(0, 2).map(([, date, slot], index) => ({
    date,
    slot_id: slot,
    items: [
      index === 0
        ? { recipe_id: recipe, new_recipe: null, lot_id: null }
        : { recipe_id: null, new_recipe: 0, lot_id: null },
    ],
    reason: index === 0 ? 'Des légumes de saison.' : 'Une soupe légère.',
  }));
  return { meals, new_recipes: [soup] };
}

createServer((request, response) => {
  let body = '';
  request.on('data', (chunk) => (body += chunk));
  request.on('end', () => {
    if (request.method !== 'POST' || !request.url.endsWith('/chat/completions')) {
      response.writeHead(404).end();
      return;
    }
    if (request.headers.authorization !== 'Bearer cle-de-test') {
      response.writeHead(401).end();
      return;
    }
    const payload = JSON.parse(body);
    const user = payload.messages.find((m) => m.role === 'user').content;
    const kind = payload.response_format?.json_schema?.name;
    const content = kind === 'planning' ? plan(user) : soup;
    const text = JSON.stringify(content);
    if (!payload.stream) {
      response.writeHead(200, { 'content-type': 'application/json' });
      response.end(
        JSON.stringify({ choices: [{ message: { role: 'assistant', content: text } }] }),
      );
      return;
    }
    // Réponse en flux, en deux morceaux, comme un vrai fournisseur.
    const half = Math.floor(text.length / 2);
    const event = (part) =>
      `data: ${JSON.stringify({ choices: [{ delta: { content: part } }] })}\n\n`;
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    response.end(event(text.slice(0, half)) + event(text.slice(half)) + 'data: [DONE]\n\n');
  });
}).listen(3197, '127.0.0.1');
