import { initAsciiPortal } from './ascii-portal';
import { initDocsNavigation } from './docs-navigation';
import { initIntegrationTabs } from './integration-tabs';

let activeBody: HTMLElement | undefined;
let cleanups: (() => void)[] = [];

function disposePage() {
  cleanups.forEach((dispose) => dispose());
  cleanups = [];
  activeBody = undefined;
}

function initPage() {
  if (activeBody === document.body) return;
  disposePage();
  activeBody = document.body;
  const controls = [initDocsNavigation(), initIntegrationTabs()];
  document.querySelectorAll<HTMLElement>('[data-ascii-portal]').forEach((root) => {
    controls.push(initAsciiPortal(root));
  });
  cleanups = controls.filter((dispose): dispose is () => void => typeof dispose === 'function');
}

document.addEventListener('astro:before-swap', disposePage);
// Prepare controls before the new view is painted, then guard against duplicate page-load events.
document.addEventListener('astro:after-swap', initPage);
document.addEventListener('astro:page-load', initPage);
initPage();
