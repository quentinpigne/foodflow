/** Adresses à importer et favori « Envoyer à Foodflow » (R-REC-69, R-REC-72). */

/** Adresses http(s) d'un texte libre (une par ligne, ou mêlées à du texte), sans doublon, dans l'ordre. */
export function extractUrls(text: string): string[] {
  const found = text.match(/https?:\/\/[^\s<>"']+/gi) ?? [];
  // Ponctuation de fin de phrase collée à l'adresse : « voir https://…/recette. »
  const cleaned = found.map((url) => url.replace(/[.,;:!?)\]]+$/, ''));
  return [...new Set(cleaned)];
}

/** Ce qu'apporte l'ouverture de la page d'import : favori (fragment) ou partage (paramètres). */
export interface SharedImport {
  url?: string;
  json_ld?: string;
}

/** Préfixe du fragment posé par le favori : `#import=<JSON en base64url>`. */
const FRAGMENT = 'import=';

/**
 * Favori : `#import=…`, un JSON `{ url, json_ld }` en base64url (le routeur décode le fragment : du JSON brut
 * ou des paramètres n'y survivraient pas) ; partage vers la PWA (Web Share Target) : `?url=…&text=…&title=…`,
 * où l'adresse arrive souvent dans `text` (« Regarde cette recette https://… »).
 */
export function readShared(fragment: string | null, query: URLSearchParams): SharedImport | null {
  if (fragment?.startsWith(FRAGMENT)) {
    try {
      const base64 = fragment.slice(FRAGMENT.length).replace(/-/g, '+').replace(/_/g, '/');
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
      const sent = JSON.parse(new TextDecoder().decode(bytes)) as SharedImport;
      if (typeof sent.json_ld === 'string') {
        return { url: typeof sent.url === 'string' ? sent.url : undefined, json_ld: sent.json_ld };
      }
    } catch {
      // Fragment tronqué ou modifié : on l'ignore.
    }
    return null;
  }
  const url = extractUrls([query.get('url'), query.get('text')].filter(Boolean).join(' '))[0];
  return url ? { url } : null;
}

/** Code du favori : lit le JSON-LD de la page affichée et l'ouvre dans Foodflow, sans passer par le serveur. */
export function bookmarklet(origin: string): string {
  const target = JSON.stringify(`${origin}/recettes/import#${FRAGMENT}`);
  const code =
    `(()=>{const d=[...document.querySelectorAll('script[type="application/ld+json"]')].flatMap(e=>{try{return[JSON.parse(e.textContent)]}catch(x){return[]}});` +
    `if(!d.length){alert('Foodflow : cette page ne publie pas de recette lisible (schema.org).');return}` +
    `const b=new TextEncoder().encode(JSON.stringify({url:location.href,json_ld:JSON.stringify(d)}));let s='';for(const c of b)s+=String.fromCharCode(c);` +
    `window.open(${target}+btoa(s).split('+').join('-').split('/').join('_').split('=').join(''))})()`;
  return `javascript:${encodeURIComponent(code)}`;
}
