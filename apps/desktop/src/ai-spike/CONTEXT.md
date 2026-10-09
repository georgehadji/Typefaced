# apps/desktop/src/ai-spike/

Dev-only AI spike page, shown at `#/ai-spike` in development builds; production bundles
drop it (M0 Step 9.2, ADR-0009). Up: [src/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `AiSpikePage.tsx` | The page: a password field that sends the key to `ai_set_key` once and is cleared first (shows only "Key stored: yes/no"), a delete-key button, the font's family name, a prompt box with Run (disabled while running) and Stop, the streamed answer, the outcome or the failure with its cause, and an approval dialog for `set_family_name` (one at a time; declined on Stop or when the page goes away) |
| `AiSpikePage.test.tsx` | Tests for the page with the bindings and `runAgent` mocked: key status, storing (field cleared, key sent once), errors without the key, deleting, approval given and declined, a second approval in one turn, refusal and truncation messages, failure causes, Stop, leaving the page |
| `ai-spike.css` | Styles for the page and the approval dialog |
