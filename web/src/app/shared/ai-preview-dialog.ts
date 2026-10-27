import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { firstValueFrom } from 'rxjs';

import { AiPreview } from '../api/models/ai-preview';

/**
 * Ce qui part vers l'IA, montré avant l'envoi et à confirmer (ADR-0007, R-REC-81, R-PLA-66) : exactement le
 * texte envoyé, membres désignés par une lettre.
 */
@Component({
  selector: 'app-ai-preview-dialog',
  imports: [MatButtonModule, MatDialogModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <h2 mat-dialog-title>Envoyer à l'IA ?</h2>
    <mat-dialog-content>
      <p class="muted">
        Voici exactement ce qui sera envoyé au fournisseur d'IA. Les membres du foyer y sont
        désignés par une lettre ; aucun nom n'est transmis.
      </p>
      <pre class="sent">{{ data.user }}</pre>
      <details>
        <summary>Consignes générales</summary>
        <pre class="sent">{{ data.system }}</pre>
      </details>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button [mat-dialog-close]="false">Annuler</button>
      <button mat-flat-button [mat-dialog-close]="true">Envoyer</button>
    </mat-dialog-actions>
  `,
  styles: `
    .sent {
      white-space: pre-wrap;
      font: var(--mat-sys-body-small);
      background: var(--mat-sys-surface-container);
      padding: 0.75rem;
      border-radius: var(--mat-sys-corner-small);
      max-height: 50vh;
      overflow: auto;
    }
    summary {
      cursor: pointer;
      margin: 0.5rem 0;
    }
  `,
})
export class AiPreviewDialog {
  protected readonly data = inject<AiPreview>(MAT_DIALOG_DATA);
}

/** Montre l'aperçu et renvoie `true` si l'utilisateur confirme l'envoi. */
export async function confirmAiSend(dialog: MatDialog, preview: AiPreview): Promise<boolean> {
  const ref = dialog.open(AiPreviewDialog, { data: preview, width: '44rem', maxWidth: '95vw' });
  return (await firstValueFrom(ref.afterClosed())) === true;
}
