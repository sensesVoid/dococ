## Facts

- HuggingFace and Kaggle tokens were stored in plaintext in `opencode.json` and were exposed during this session.
- The user decided to leave the tokens as-is because they are unused but valid.
- No secrets may be stored in `.agent_context/memory/` or `AGENTS.md`.

## Decisions

- Leave the existing tokens as-is; no rotation.
- Use the tokens only through the configured MCP/CLI integrations.
- Store any future tokens in environment variables or the OS keyring.
- The LLM gets tickers and relative exposure only; never balances or account identifiers.
- News is untrusted data; the LLM gets no tools and must return strict JSON.
- Trading API keys must be withdrawal-disabled and IP-allowlisted where possible.

## Gotchas

- Do not print, copy, or paste tokens into chat, prompts, logs, or memory files.
- Treat the exposed tokens as an accepted risk; do not share `opencode.json`.
- Revisit the decision if the tokens are granted broader scopes or the config is shared.
