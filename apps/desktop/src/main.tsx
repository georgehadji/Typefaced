import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

const root = ReactDOM.createRoot(
  document.getElementById("root") as HTMLElement,
);

function render() {
  // The benchmark page exists only in development builds (M0 Step 8); Vite drops this
  // branch, and the bench code with it, from production bundles.
  if (import.meta.env.DEV && window.location.hash === "#/bench") {
    void import("./bench/BenchPage").then(({ default: BenchPage }) =>
      root.render(<BenchPage />),
    );
    return;
  }
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

render();
if (import.meta.env.DEV) {
  window.addEventListener("hashchange", render);
}
