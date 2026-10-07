// The two demo tools of Spike 4 (M0 Step 9.2): `get_font_summary` (read-only) and
// `set_family_name` (changes the in-memory font, only after the user approves). Every
// tool input is validated against its JSON Schema with Ajv before the tool runs: the
// model's input is untrusted (a fallback model may not be Anthropic's own endpoint), and
// a turn cut off mid-input can hand over a truncated object.
//
// ponytail: Ajv compiles validators with `new Function`, which the app's CSP (no
// 'unsafe-eval') blocks. That is fine for the dev-only `#/ai-spike` page (Tauri does
// not apply the CSP under `tauri dev`); production code needs Ajv standalone
// (precompiled) validators.
import { betaTool } from "@anthropic-ai/sdk/helpers/beta/json-schema";
import type { BetaRunnableTool } from "@anthropic-ai/sdk/lib/tools/BetaRunnableTool";
import { ToolError } from "@anthropic-ai/sdk/lib/tools/ToolError";
import Ajv from "ajv";

export interface FontSummary {
  familyName: string;
  styleName: string;
  unitsPerEm: number;
  glyphCount: number;
}

/** `tests/fixtures/min.ufo` as a summary. */
export const FIXTURE_FONT: FontSummary = {
  familyName: "Typefaced Test",
  styleName: "Regular",
  unitsPerEm: 1000,
  glyphCount: 6,
};

/** The in-memory font. `set` replaces the whole value; nothing is mutated in place. */
export interface FontStore {
  get(): FontSummary;
  set(next: FontSummary): void;
  subscribe(listener: (font: FontSummary) => void): () => void;
}

export function createFontStore(initial: FontSummary): FontStore {
  let current = initial;
  const listeners = new Set<(font: FontSummary) => void>();
  return {
    get: () => current,
    set(next) {
      current = next;
      for (const listener of listeners) listener(next);
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export interface ApprovalRequest {
  tool: "set_family_name";
  from: string;
  to: string;
}

/** Asks the user; resolves `true` only if they approved. */
export type Approve = (request: ApprovalRequest) => Promise<boolean>;

const MAX_FAMILY_NAME = 63;

const SUMMARY_SCHEMA = {
  type: "object",
  properties: {},
  additionalProperties: false,
} as const;

const RENAME_SCHEMA = {
  type: "object",
  properties: {
    family_name: {
      type: "string",
      minLength: 1,
      maxLength: MAX_FAMILY_NAME,
      // No control or format characters (newlines, bidi overrides) in the approval
      // dialog or the font. Ajv compiles patterns with the `u` flag.
      pattern: "^\\P{C}+$",
      description: "The new family name, e.g. Test Sans",
    },
  },
  required: ["family_name"],
  additionalProperties: false,
} as const;

const ajv = new Ajv();

/** Returns a check that throws the `INVALID_JSON` tool error for input not matching `schema`. */
function validator(schema: object) {
  const validate = ajv.compile(schema);
  return (input: unknown) => {
    if (!validate(input)) {
      throw new ToolError(
        JSON.stringify({
          INVALID_JSON: JSON.stringify(input),
          error: ajv.errorsText(validate.errors),
        }),
      );
    }
  };
}

export function fontTools(
  store: FontStore,
  approve: Approve,
): BetaRunnableTool[] {
  const checkSummary = validator(SUMMARY_SCHEMA);
  const checkRename = validator(RENAME_SCHEMA);
  const summary = betaTool({
    name: "get_font_summary",
    description:
      "Read the open font's family name, style name, units per em and glyph count. " +
      "Call this before answering any question about the font.",
    inputSchema: SUMMARY_SCHEMA,
    run: async (input) => {
      checkSummary(input);
      return JSON.stringify(store.get());
    },
  });
  const rename = betaTool({
    name: "set_family_name",
    description:
      "Change the open font's family name. Call this when the user asks to rename the " +
      "family. The user must approve the change; if they decline, nothing changes.",
    inputSchema: RENAME_SCHEMA,
    run: async (input) => {
      checkRename(input);
      const from = store.get().familyName;
      const to = input.family_name;
      if (!(await approve({ tool: "set_family_name", from, to }))) {
        return "The user declined the change. The family name is unchanged.";
      }
      store.set({ ...store.get(), familyName: to });
      return `The family name is now ${to}.`;
    },
  });
  // No `eager_input_streaming`: OpenRouter's Messages API reference does not list it.
  return [summary, rename];
}
