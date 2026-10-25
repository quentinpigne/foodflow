import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatListModule } from '@angular/material/list';
import { MatMenuModule } from '@angular/material/menu';
import { MatSidenavModule } from '@angular/material/sidenav';
import { MatToolbarModule } from '@angular/material/toolbar';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';

import { Session } from '../../core/auth';
import { Layout } from '../../core/layout';
import { AddLot } from '../stock/add-lot';

interface NavItem {
  path: string;
  label: string;
  icon: string;
  exact: boolean;
  /** Absent de la barre du bas (mobile) : dans le menu. */
  secondary?: boolean;
}

/** Navigation : barre du bas et « + » sur mobile, menu latéral permanent sur ordinateur (comme Cashflow). */
@Component({
  selector: 'app-shell',
  imports: [
    RouterOutlet,
    RouterLink,
    RouterLinkActive,
    MatSidenavModule,
    MatListModule,
    MatMenuModule,
    MatToolbarModule,
    MatIconModule,
    MatButtonModule,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './shell.html',
  styleUrl: './shell.scss',
})
export class Shell {
  protected readonly layout = inject(Layout);
  private readonly addLot = inject(AddLot);
  private readonly session = inject(Session);
  private readonly router = inject(Router);

  protected readonly items: NavItem[] = [
    { path: '/', label: 'Accueil', icon: 'home', exact: true },
    { path: '/stock', label: 'Stock', icon: 'kitchen', exact: false },
    { path: '/recettes', label: 'Recettes', icon: 'menu_book', exact: false },
    { path: '/aliments', label: 'Aliments', icon: 'nutrition', exact: false },
    { path: '/objectifs', label: 'Objectifs', icon: 'target', exact: false, secondary: true },
    { path: '/reglages', label: 'Réglages', icon: 'settings', exact: false },
  ];

  protected readonly primaryItems = this.items.filter((item) => !item.secondary);

  protected newLot(): void {
    this.addLot.open();
  }

  protected async logout(): Promise<void> {
    await this.session.logout();
    await this.router.navigate(['/connexion']);
  }
}
