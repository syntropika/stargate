/** Initialize the guide controls for the current document body. */
export function initDocsNavigation() {
  document
    .querySelectorAll<HTMLElement>('.doc-content pre, .doc-content table')
    .forEach((block) => {
      block.tabIndex = 0;
    });
  const toggle = document.querySelector<HTMLButtonElement>('.docs-toggle');
  const navigation = document.querySelector<HTMLElement>('#docs-navigation');
  if (!toggle || !navigation) return;

  const controller = new AbortController();
  const { signal } = controller;
  const mobile = window.matchMedia('(max-width: 45rem)');
  const setOpen = (open: boolean) => {
    toggle.setAttribute('aria-expanded', String(open));
    navigation.classList.toggle('is-open', open);
  };
  toggle.hidden = false;
  document.body.classList.add('docs-menu-ready');
  toggle.addEventListener('click', () => setOpen(toggle.getAttribute('aria-expanded') !== 'true'), {
    signal,
  });
  document.addEventListener(
    'keydown',
    (event) => {
      if (
        event.key === 'Escape' &&
        mobile.matches &&
        toggle.getAttribute('aria-expanded') === 'true'
      ) {
        setOpen(false);
        toggle.focus();
      }
    },
    { signal },
  );
  mobile.addEventListener(
    'change',
    () => {
      if (navigation.contains(document.activeElement) || document.activeElement === toggle) {
        if (mobile.matches) toggle.focus();
        else navigation.querySelector<HTMLAnchorElement>('a[aria-current]')?.focus();
      }
      setOpen(false);
    },
    { signal },
  );
  return () => controller.abort();
}
