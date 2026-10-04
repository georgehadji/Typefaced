# apps/desktop/src/

React UI of the desktop app. It talks to Rust only through the generated
`@typefaced/bindings` package. Up: [apps/desktop/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `main.tsx` | Entry point: renders `<App />` into `#root` in `React.StrictMode`; in development builds, `#/bench` renders the benchmark page instead (excluded from coverage) |
| `App.tsx` | Root component: calls `commands.appInfo()` and shows "name vVersion", or an alert with the reason when the call fails; ignores results that arrive after unmount |
| `App.test.tsx` | Vitest + Testing Library tests for `App` with the bindings mocked: success, `Error` and string rejections, late results after unmount |
| `App.css` | Global styles: light and dark color scheme, centered layout |
| `vite-env.d.ts` | Vite client type references; declares `VITE_BENCH_ONLY` (`drag` runs only the drag benchmarks on `#/bench`) |

| Subfolder | Context |
|---|---|
| `bench/` | [CONTEXT.md](bench/CONTEXT.md): dev-only IPC and WASM benchmark page |
