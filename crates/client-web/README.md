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

Run the local SonarJS checks:

```cmd
npm run lint
```

This checks cognitive complexity, nested conditionals, nested template literals, and multiline control-flow blocks.
It covers client source, tests, and tooling configuration. It runs in CI before type-checking, tests, and the build.
SonarCloud still runs the full hosted analysis; the local checks cover a focused subset of its rules.
The SonarJS dependency override keeps its legacy compiler API separate from the TypeScript 7 native compiler used
by `npm run check`. Babel parses TypeScript syntax for linting without changing the application build.

Before opening a pull request, run all frontend checks and the production build:

```cmd
npm run validate
```

This uses `npm-run-all2` to run `lint`, `check`, `test`, and `build` in order, stopping on failure.
CI uses the same command after `npm ci --ignore-scripts`. Each check is also available separately.

## Browse navigation

On Home, library, item, collection, and person pages, arrow keys and the controller D-pad or left stick move to nearby controls in the visible layout. Cards scroll into view automatically. Moving left from the edge of a content section enters the library sidebar; moving right from the sidebar returns to the last content section.

Moving Up or Down between media sections starts at the first card on an unvisited row and restores the last focused card on a previously visited row. This also applies when moving down from the featured item's Open button.

Use **Tab** / **Shift+Tab** or the controller **RB** / **LB** buttons to jump to the next or previous section. Each section remembers its last focused control. Inside forms, Tab moves between fields before leaving for the next section. **Enter** or controller **A** opens the focused control; controller **B** goes back. Arrow keys still edit text fields and native selects. Settings, authentication screens, and players retain native Tab behavior.

From search, **Escape** returns to the last focused browse tab and keeps the query. **Left** does the same when the caret is at the start of the search text. Controller **Left** from the search field or **B** from either search control also returns to the browse tabs.

Run the frontend behavior tests:

```cmd
npm test
```

## Real-browser playback tests

Install Node.js 26, the repository's Rust toolchain, and FFmpeg with the `libx264` and AAC
encoders. The fixture generator also needs the `lavfi` input device. Install Chrome, or run
`npx playwright install --with-deps chrome` from this directory.

Run `npm run test:e2e`. This builds the production web bundle and a server with
`--no-default-features`, generates original video/audio/subtitle fixtures, and starts Koko with
a temporary configuration and database. The suite exercises actual HTML media decoding and
server FFmpeg conversion. Test failures retain screenshots, traces, and an HTML report.

Set `KOKO_E2E_BROWSER=firefox` and run `npx playwright install --with-deps firefox` to test Firefox.

Optional environment variables:

| Variable                    | Purpose                                                     |
|-----------------------------|-------------------------------------------------------------|
| `KOKO_E2E_BROWSER`          | Browser engine, `chromium` (default) or `firefox`.          |
| `KOKO_E2E_CHANNEL`          | Chromium browser channel; defaults to `chrome`.             |
| `KOKO_E2E_FFMPEG`           | FFmpeg executable used by the server and encoder fixture.   |
| `KOKO_E2E_FFPROBE`          | ffprobe executable used to scan fixtures.                   |
| `KOKO_E2E_GENERATOR_FFMPEG` | Separate FFmpeg with `lavfi` for generated source media.    |
| `KOKO_E2E_PORT`             | Server port, default `19192`; readiness uses the next port. |
| `KOKO_E2E_SERVER`           | Existing server executable to use instead of building one.  |

Use a browser channel with H.264/AAC support. Ordinary component tests remain available through
`npm test` and are separate from this server-backed suite.
