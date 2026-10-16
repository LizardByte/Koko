import { readFile } from 'node:fs/promises';
import { defineConfig, type PreviewServer, type ViteDevServer } from 'vite';

/** Serve backend-owned images when developing or previewing without a Rust server. */
function serveBackendAssets(server: ViteDevServer | PreviewServer): void {
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
    if (!/^[a-z0-9-]+\.(svg|png)$/.test(filename)) {
      response.statusCode = 404;
      response.end();
      return;
    }
    try {
      const bytes = await readFile(new URL(filename, assetDirectory));
      response.setHeader('Content-Type', filename.endsWith('.svg') ? 'image/svg+xml' : 'image/png');
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
