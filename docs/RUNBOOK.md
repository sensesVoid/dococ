# Doc Oc — Incident Runbook

Owner: @Geh. Review after every incident.

## 1. Kill (halt everything)

1. `dococ kill` — flattens all positions and halts the daemon.
2. If the daemon is unresponsive, stop the process (`taskkill /PID` or `systemctl stop dococ`).
3. Verify no open orders remain at the venue.

## 2. Flatten a single position

1. `dococ reconcile` — compare venue truth against the local ledger.
2. Any orphaned position (venue yes, ledger no): treat the venue as truth,
   re-enter it, then post-mortem.

## 3. Reconcile

1. `dococ reconcile` must report zero differences.
2. If differences persist: freeze trading, export logs + decision journal,
   and do not paper/live trade until ledger equals venue truth.

## 4. Stale LLM brief (over 30 minutes)

- New entries are blocked automatically (stale-brief fallback).
- If the LLM is down: trading continues on the last valid brief; do not
  override the guardian or manually retrain.

## 5. Inference timeout (over 10s warm)

- The action degrades to `NO_ACTION`; count these in the drift report.
- If timeouts exceed 5% of decisions, run `scripts/run_laya_bench.ps1` and
  investigate CPU contention before trusting any trade.

## 6. Post-mortem checklist

- [ ] Timeline of events (logs + decision journal + drift report)
- [ ] Root cause (model / data / risk / infra)
- [ ] Was the guardian breached? (must never be)
- [ ] Action items with owners and dates
