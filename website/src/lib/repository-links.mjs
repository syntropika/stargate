// Keep checked-in documentation usable both on GitHub and on the public website.
export function repositoryLinks() {
  return (tree) => {
    const walk = (node) => {
      if (
        node.type === 'link' &&
        typeof node.url === 'string' &&
        !/^(?:[a-z]+:|\/|#)/i.test(node.url)
      ) {
        const [path, fragment] = node.url.split('#');
        if (path.endsWith('.md') && !path.includes('/')) {
          node.url = `/docs/${path.slice(0, -3)}/${fragment ? '#' + fragment : ''}`;
        } else {
          const relative = path.startsWith('../') ? path.slice(3) : 'docs/' + path;
          node.url = `https://github.com/syntropika/stargate/blob/main/${relative}${fragment ? '#' + fragment : ''}`;
        }
      }
      for (const child of node.children || []) walk(child);
    };
    walk(tree);
  };
}
