// `@typefaced/ai`: the Claude client for the webview, via OpenRouter, over the Rust egress
// proxy (ADR-0009).
export {
  type AgentOptions,
  type AgentOutcome,
  createClaudeClient,
  KEY_PLACEHOLDER,
  OPENROUTER_BASE_URL,
  ROUTES,
  type Route,
  runAgent,
  type Task,
} from "./agent";
export {
  type ApprovalRequest,
  type Approve,
  createFontStore,
  FIXTURE_FONT,
  type FontStore,
  type FontSummary,
  fontTools,
} from "./fontTools";
export { tauriFetch } from "./tauriFetch";
