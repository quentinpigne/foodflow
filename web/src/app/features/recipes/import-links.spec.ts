import { bookmarklet, extractUrls, readShared } from './import-links';

describe('extractUrls (R-REC-69)', () => {
  it('lit une adresse par ligne, sans doublon ni ponctuation finale', () => {
    const text = `https://a.test/r/1
      voir https://a.test/r/2.
      https://a.test/r/1
      (https://b.test/x?y=1)`;
    expect(extractUrls(text)).toEqual([
      'https://a.test/r/1',
      'https://a.test/r/2',
      'https://b.test/x?y=1',
    ]);
  });

  it('ignore ce qui n’est pas une page web', () => {
    expect(extractUrls('ftp://a.test et rien')).toEqual([]);
  });
});

describe('readShared (R-REC-72)', () => {
  it('reprend le JSON-LD et l’adresse envoyés par le favori, accents et & compris', () => {
    const sent = { url: 'https://a.test/r?a=1&b=2', json_ld: '[{"name":"Crêpes & cidre"}]' };
    const bytes = new TextEncoder().encode(JSON.stringify(sent));
    const base64 = btoa(String.fromCharCode(...bytes))
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=+$/, '');
    expect(readShared(`import=${base64}`, new URLSearchParams())).toEqual(sent);
  });

  it('ignore un fragment abîmé', () => {
    expect(readShared('import=%%%', new URLSearchParams())).toBeNull();
  });

  it('trouve l’adresse d’un partage, même dans le texte', () => {
    const query = new URLSearchParams({ title: 'Crêpes', text: 'Regarde https://a.test/crepes' });
    expect(readShared(null, query)).toEqual({ url: 'https://a.test/crepes' });
  });

  it('ne fait rien sans adresse ni données', () => {
    expect(readShared(null, new URLSearchParams())).toBeNull();
  });
});

describe('bookmarklet (R-REC-72)', () => {
  it('ouvre la page d’import de cette instance', () => {
    const href = bookmarklet('https://foodflow.test');
    expect(href.startsWith('javascript:')).toBe(true);
    const code = decodeURIComponent(href.slice('javascript:'.length));
    expect(code).toContain('"https://foodflow.test/recettes/import#import="');
    expect(code).not.toContain('\n');
  });
});
