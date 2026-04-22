# Koko web client

This is the initial browser client shell for Koko.

Current scope:

- configurable API base URL
- automatic mock-data fallback during local development when the server is unavailable
- server capability bootstrap
- media library list
- media item list
- media item detail view
- simple search against the Stage 1 media APIs

## Development

Install dependencies:

```cmd
npm install
```

Start the dev server:

```cmd
npm run dev
```

In development mode, the client will automatically fall back to mock data if the Koko server is unavailable.

Start the dev server in forced mock mode:

```cmd
npm run dev:mock
```

Use a custom backend base URL during development:

```cmd
set VITE_API_BASE_URL=https://127.0.0.1:9191
npm run dev
```

Build the client:

```cmd
npm run build
```

After building, the Rust server serves the generated bundle from `crates/client-web/dist`.

Type-check the client:

```cmd
npm run check
```



