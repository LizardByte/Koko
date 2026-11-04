/** Bundles the client and serves backend artwork and local reader worker assets in development. */
import { cp, mkdir, readFile } from 'node:fs/promises';
import { defineConfig, type PreviewServer, type ViteDevServer } from 'vite';

/** Serve backend-owned images when developing or previewing without a Rust server. */
function serveBackendAssets(server: ViteDevServer | PreviewServer): void {
  server.middlewares.use('/assets/reader', async (request, response) => {
    let path: string;
    try { path = decodeURIComponent(new URL(request.url ?? '/', 'http://localhost').pathname).slice(1); }
    catch { response.statusCode = 400; response.end(); return; }
    let root = '';
    if (path.startsWith('libarchive/')) root = 'libarchive.js/dist/';
    else if (path.startsWith('pdfjs/')) root = 'pdfjs-dist/';
    const relative = path.slice(path.indexOf('/') + 1);
    if (!root || !/^[\w./-]+$/.test(relative) || relative.split('/').includes('..')) { response.statusCode = 404; response.end(); return; }
    try {
      const bytes = await readFile(new URL(`node_modules/${root}${relative}`, import.meta.url));
      let type = 'application/octet-stream';
      if (relative.endsWith('.wasm')) type = 'application/wasm';
      else if (relative.endsWith('.js')) type = 'text/javascript';
      response.setHeader('Content-Type', type);
      response.end(bytes);
    } catch { response.statusCode = 404; response.end(); }
  });
  server.middlewares.use('/favicon.ico', async (request, response, next) => {
    if (new URL(request.url ?? '/', 'http://localhost').pathname !== '/') {
      next();
      return;
    }
    try {
      const bytes = await readFile(new URL('../../assets/icon.ico', import.meta.url));
      response.setHeader('Content-Type', 'image/x-icon');
      response.end(bytes);
    } catch {
      response.statusCode = 404;
      response.end();
    }
  });
  const assetDirectory = new URL('../../assets/providers/', import.meta.url);
  server.middlewares.use('/assets/providers', async (request, response) => {
    const filename = new URL(request.url ?? '/', 'http://localhost').pathname.slice(1);
    if (!/^[a-z0-9-]+\.(svg|png|ico)$/.test(filename)) {
      response.statusCode = 404;
      response.end();
      return;
    }
    try {
      const bytes = await readFile(new URL(filename, assetDirectory));
      const contentTypes:Record<string,string>={svg:'image/svg+xml',ico:'image/x-icon',png:'image/png'};
      response.setHeader('Content-Type', contentTypes[filename.split('.').at(-1)!]);
      response.end(bytes);
    } catch {
      response.statusCode = 404;
      response.end();
    }
  });
}

export default defineConfig({
  plugins: [{
    name: 'backend-assets',
    configureServer: serveBackendAssets,
    configurePreviewServer: serveBackendAssets,
    async closeBundle() {
      const output = new URL('dist/assets/reader/', import.meta.url);
      await mkdir(output, { recursive: true });
      await Promise.all(['cmaps', 'standard_fonts', 'wasm'].map((folder) =>
        cp(new URL(`node_modules/pdfjs-dist/${folder}/`, import.meta.url), new URL(`pdfjs/${folder}/`, output), { recursive: true })));
      await mkdir(new URL('libarchive/', output), { recursive: true });
      await Promise.all(['worker-bundle.js', 'libarchive.wasm'].map((file) =>
        cp(new URL(`node_modules/libarchive.js/dist/${file}`, import.meta.url), new URL(`libarchive/${file}`, output))));
    },
  }],
  server: {
    host: '127.0.0.1',
    port: 4173,
  },
  preview: {
    host: '127.0.0.1',
    port: 4173,
  },
});
