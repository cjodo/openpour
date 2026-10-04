// Builds the app into dist/, which the firmware embeds (firmware/esp32/build.rs).
//   node build.mjs        production: type-checked by `npm run build`, minified, no simulator
//   node build.mjs --dev  serves http://localhost:8000/?mock (PORT=… to change), rebuilding on each request

import * as esbuild from 'esbuild';
import { cp, mkdir, rm } from 'node:fs/promises';

const dev = process.argv.includes('--dev');
const STATIC = ['index.html', 'style.css', 'icon.svg', 'manifest.webmanifest', 'default-recipes.json'];

const options = {
  entryPoints: ['src/app.ts'],
  bundle: true,
  format: 'esm',
  target: 'es2022',
  define: { __SIMULATOR__: String(dev) },
  logLevel: 'info',
};

if (dev) {
  // Output is served from memory next to the static files in this folder.
  const ctx = await esbuild.context({ ...options, outdir: '.', write: false, sourcemap: 'inline' });
  const { port } = await ctx.serve({ servedir: '.', port: +(process.env.PORT ?? 8000) });
  console.log(`\n  Simulator: http://localhost:${port}/?mock  (add &speed=4 to fast-forward)\n`);
} else {
  await rm('dist', { recursive: true, force: true });
  await mkdir('dist');
  await Promise.all(STATIC.map((f) => cp(f, `dist/${f}`)));
  await esbuild.build({ ...options, outdir: 'dist', minify: true });
}
