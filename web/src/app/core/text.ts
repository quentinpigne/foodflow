/** Texte comparable : minuscules, sans accents (« Crème » → « creme »). */
export function fold(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}
