/** Keep installation commands and application examples on the same language. */
let selectedLanguage = 'node';

export function initIntegrationTabs() {
  const installation = document.querySelector<HTMLElement>('[data-installation]');
  if (!installation) return;
  const controller = new AbortController();
  const { signal } = controller;
  const groups = [...document.querySelectorAll<HTMLElement>('[data-language-tabs]')].map(
    (root) => ({
      tabs: [...root.querySelectorAll<HTMLButtonElement>('[role="tab"]')],
      panels: [...root.querySelectorAll<HTMLElement>('[role="tabpanel"]')],
    }),
  );
  const status = installation.querySelector<HTMLElement>('.copy-status')!;
  const guide = installation.querySelector<HTMLAnchorElement>('[data-integration-guide]')!;
  let selection = 0;

  const activate = (language: string) => {
    selectedLanguage = language;
    selection++;
    for (const { tabs, panels } of groups) {
      const selected = tabs.find((tab) => tab.dataset.language === language);
      if (!selected) continue;
      for (const tab of tabs) {
        tab.setAttribute('aria-selected', String(tab === selected));
        tab.tabIndex = tab === selected ? 0 : -1;
      }
      for (const panel of panels)
        panel.hidden = panel.id !== selected.getAttribute('aria-controls');
    }
    const tab = installation.querySelector<HTMLButtonElement>('[aria-selected="true"]')!;
    guide.href = `/docs/integrations/#${tab.dataset.guide}`;
    guide.firstChild!.textContent = `${tab.textContent} integration guide `;
    status.textContent = '';
  };

  for (const { tabs } of groups) {
    tabs.forEach((tab, index) => {
      tab.addEventListener('click', () => activate(tab.dataset.language!), { signal });
      tab.addEventListener(
        'keydown',
        (event) => {
          let next: number;
          switch (event.key) {
            case 'ArrowRight':
              next = (index + 1) % tabs.length;
              break;
            case 'ArrowLeft':
              next = (index + tabs.length - 1) % tabs.length;
              break;
            case 'Home':
              next = 0;
              break;
            case 'End':
              next = tabs.length - 1;
              break;
            default:
              return;
          }
          event.preventDefault();
          activate(tabs[next].dataset.language!);
          tabs[next].focus();
        },
        { signal },
      );
    });
  }

  installation.querySelectorAll<HTMLButtonElement>('.copy-command').forEach((button) => {
    button.addEventListener(
      'click',
      async () => {
        const attempt = selection;
        const command = button.parentElement!.querySelector('code')!.textContent!;
        button.disabled = true;
        try {
          await navigator.clipboard.writeText(command);
          if (!signal.aborted && selection === attempt) status.textContent = 'Command copied.';
        } catch {
          if (!signal.aborted && selection === attempt)
            status.textContent = 'Could not copy. Select the command and copy it manually.';
        } finally {
          button.disabled = false;
        }
      },
      { signal },
    );
  });
  activate(selectedLanguage);
  return () => controller.abort();
}
