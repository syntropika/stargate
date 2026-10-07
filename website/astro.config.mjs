import { defineConfig } from 'astro/config';
import { unified } from '@astrojs/markdown-remark';
import { repositoryLinks } from './src/lib/repository-links.mjs';

export default defineConfig({
  site: 'https://stargate.syntropika.ai',
  output: 'static',
  trailingSlash: 'always',
  vite: { build: { assetsInlineLimit: 0 } },
  markdown: { processor: unified({ remarkPlugins: [repositoryLinks] }), syntaxHighlight: false },
});
