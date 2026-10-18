import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  // Relative asset paths, so the site works under /gb-strudel/ on GitHub Pages
  // (or any other subpath) without hard-coding the repository name.
  base: './',
  // The example songs are imported from ../songs.
  server: { fs: { allow: ['..'] } },
});
