## Facts

- All design questions are closed in `Open_Questions.md`.
- Authority: Laya decides; the guardian enforces hard risk limits; the LLM has no order authority.
- Risk defaults: 0.25% equity risk per trade, 2% daily loss, 5% max drawdown, 1x notional cap.
- Promotion: 2 weeks shadow + 4 weeks paper or 500 decisions, whichever is longer.
- Data: free-first.
- SQAC: original notes only; no proprietary course text.
- Deployment: Windows development; Linux VPS 8 vCPU/16 GB production.

## Decisions

- Paper-first generic venue path; live BTC first, then gold.
- OpenCode CLI subprocess for the 15-minute LLM brief.
- Guardian can reject, cap, or flatten Laya's orders.
- Manual checkpoint promotion; auto-rollback on drift or rule breach.
- 15-minute decisions; 1-minute execution and stop management.

## Gotchas

- Live adapters are deferred; paper results are not live readiness.
- Funded-firm automation permission is a go/no-go.
- The LLM brief is a feature and guardian input, not a historical training label.
