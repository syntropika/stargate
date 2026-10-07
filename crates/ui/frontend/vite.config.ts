import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import { fileURLToPath } from 'node:url';
import { readFileSync } from 'node:fs';

export default defineConfig({
  plugins: [tailwindcss()],
  define: { 'process.env.NODE_ENV': JSON.stringify('production') },
  build: {
    outDir: '../assets',
    emptyOutDir: false,
    cssCodeSplit: false,
    rolldownOptions: {
      output: {
        comments: { legal: true },
        banner:
          '/*!\n' +
          readFileSync(new URL('../assets/THIRD_PARTY_LICENSES.txt', import.meta.url), 'utf8') +
          '\n*/',
      },
    },
    lib: {
      entry: fileURLToPath(new URL('./client.tsx', import.meta.url)),
      formats: ['iife'],
      name: 'StargateAccount',
      fileName: () => 'app.js',
      cssFileName: 'app',
    },
  },
});
