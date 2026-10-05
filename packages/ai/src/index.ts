// `@typefaced/ai`: the Claude client for the webview over the Rust egress proxy (ADR-0009).
export {
  type AgentOptions,
  type AgentOutcome,
  createClaudeClient,
  FALLBACK_BETA,
  KEY_PLACEHOLDER,
  MODEL,
  runAgent,
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
