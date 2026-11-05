"""Phase 0.3 teacher spike.

Causal pipeline:
  * GARCH(1,1) volatility forecast, refit on a rolling window.
  * 3-state Gaussian HMM regime, refit on a rolling window; only the last
    (filtered) posterior of each fit is used, then held until the next refit.
  * Deterministic risk-adjusted momentum + regime score.
  * Cost-adjusted 3-class soft labels: LONG / SHORT / NO_ACTION.
  * Net-of-cost backtest against buy-and-hold.

This is a research spike, not the production teacher. It writes a label CSV
for the later Laya fine-tune.
"""

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd
from arch import arch_model
from hmmlearn.hmm import GaussianHMM

ANN_FACTOR = 24 * 365


def load_data(path: Path) -> pd.DataFrame:
    frame = pd.read_csv(path)
    frame["open_time"] = pd.to_datetime(frame["open_time"], unit="ms", utc=True)
    frame = frame.sort_values("open_time").set_index("open_time")
    for column in ("open", "high", "low", "close", "volume"):
        frame[column] = pd.to_numeric(frame[column], errors="coerce")
    frame = frame.dropna(subset=["close"])
    frame["log_return"] = np.log(frame["close"]).diff()
    frame["realized_vol_24"] = frame["log_return"].rolling(24).std() * math.sqrt(24)
    return frame


def rolling_garch(returns_pct: pd.Series, window: int, refit_every: int, horizon: int) -> pd.Series:
    values = returns_pct.to_numpy(dtype=float)
    n = len(values)
    vol = np.full(n, np.nan)

    for end in range(window - 1, n, refit_every):
        train = values[end - window + 1 : end + 1]
        if not np.all(np.isfinite(train)):
            continue
        try:
            model = arch_model(
                train,
                mean="Zero",
                vol="Garch",
                p=1,
                q=1,
                dist="t",
                rescale=False,
            )
            result = model.fit(disp="off", show_warning=False)
            forecast = result.forecast(horizon=horizon, reindex=False)
            variances = forecast.variance.iloc[0].to_numpy(dtype=float)
            for step, variance in enumerate(variances, start=1):
                index = end + step
                if index < n and np.isfinite(variance) and variance > 0:
                    vol[index] = math.sqrt(variance) / 100.0
        except Exception as exc:  # noqa: BLE001 - a failed fit must not stop the walk-forward
            print(f"garch fit failed at {end}: {exc}", file=sys.stderr)

    return pd.Series(vol, index=returns_pct.index, name="garch_vol")


def rolling_hmm(features: pd.DataFrame, window: int, refit_every: int) -> pd.DataFrame:
    values = features.to_numpy(dtype=float)
    n = len(values)
    posteriors = np.full((n, 3), np.nan)

    for end in range(window - 1, n, refit_every):
        train = values[end - window + 1 : end + 1]
        if not np.all(np.isfinite(train)):
            continue
        mean = train.mean(axis=0)
        scale = train.std(axis=0)
        scale[scale == 0] = 1.0
        standardized = (train - mean) / scale
        try:
            model = GaussianHMM(
                n_components=3,
                covariance_type="diag",
                n_iter=100,
                tol=1e-3,
                random_state=42,
            )
            model.fit(standardized)
            last = model.predict_proba(standardized)[-1]
            # Label states by their mean log-return: low -> down, high -> up.
            order = np.argsort(model.means_[:, 0])
            mapped = np.zeros(3)
            for rank, state in enumerate(order):
                mapped[rank] = last[state]
            posteriors[end] = mapped
        except Exception as exc:  # noqa: BLE001
            print(f"hmm fit failed at {end}: {exc}", file=sys.stderr)

    frame = pd.DataFrame(
        posteriors,
        index=features.index,
        columns=["p_down", "p_range", "p_up"],
    )
    return frame.ffill()


def sigmoid(value: np.ndarray) -> np.ndarray:
    return 1.0 / (1.0 + np.exp(-value))


def build_signals(
    frame: pd.DataFrame,
    garch_vol: pd.Series,
    regimes: pd.DataFrame,
    horizon: int,
    cost_round_trip: float,
    temperature: float,
) -> pd.DataFrame:
    work = frame.copy()
    work["garch_vol"] = garch_vol
    work = work.join(regimes)
    work["mom_24"] = work["close"].pct_change(24)
    work["mom_72"] = work["close"].pct_change(72)

    volatility = work["garch_vol"].clip(lower=1e-9)
    work["trend_z"] = 0.6 * work["mom_24"] / volatility + 0.4 * work["mom_72"] / (
        volatility * math.sqrt(3.0)
    )
    work["regime_z"] = work["p_up"] - work["p_down"]
    work["edge"] = 0.5 * work["trend_z"] + 0.5 * work["regime_z"]
    work["cost_z"] = cost_round_trip / volatility
    work["net_edge"] = work["edge"] - work["cost_z"]

    logits = np.column_stack(
        [
            temperature * work["net_edge"].to_numpy(),
            -temperature * work["net_edge"].to_numpy(),
            np.zeros(len(work)),
        ]
    )
    logits = logits - logits.max(axis=1, keepdims=True)
    probabilities = np.exp(logits)
    probabilities = probabilities / probabilities.sum(axis=1, keepdims=True)
    work["p_long"] = probabilities[:, 0]
    work["p_short"] = probabilities[:, 1]
    work["p_no_action"] = probabilities[:, 2]

    work["signal"] = 0
    work.loc[
        (work["p_long"] > work["p_short"] + 0.10) & (work["p_long"] > 0.40),
        "signal",
    ] = 1
    work.loc[
        (work["p_short"] > work["p_long"] + 0.10) & (work["p_short"] > 0.40),
        "signal",
    ] = -1

    work["fwd_ret_24"] = work["close"].shift(-horizon) / work["close"] - 1.0
    return work


def apply_holding_period(signal: np.ndarray, horizon: int) -> np.ndarray:
    position = np.zeros(len(signal))
    remaining = 0
    for index, desired in enumerate(signal):
        if remaining > 0:
            position[index] = position[index - 1]
            remaining -= 1
        elif desired != 0:
            position[index] = desired
            remaining = horizon
    return position


def metrics(returns: pd.Series, positions: pd.Series, turnover: pd.Series) -> dict:
    clean = returns.dropna()
    if clean.empty:
        return {}
    equity = np.exp(clean.cumsum())
    drawdown = equity / equity.cummax() - 1.0
    active = returns[positions.shift(1).fillna(0) != 0].dropna()
    return {
        "bars": int(len(clean)),
        "total_return": float(np.expm1(clean.sum())),
        "annualized_sharpe": float(clean.mean() / clean.std() * math.sqrt(ANN_FACTOR))
        if clean.std() > 0
        else 0.0,
        "max_drawdown": float(drawdown.min()),
        "bar_hit_rate": float((active > 0).mean()) if not active.empty else 0.0,
        "active_bars": int((positions != 0).sum()),
        "position_changes": int((turnover > 0).sum()),
        "average_gross_exposure": float(positions.abs().mean()),
        "total_cost": float((turnover * 0.0).sum()),  # fill below
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", default="data/btcusdt_1h.csv")
    parser.add_argument("--out", default="data/teacher_labels.csv")
    parser.add_argument("--window", type=int, default=2000)
    parser.add_argument("--refit-every", type=int, default=48)
    parser.add_argument("--horizon", type=int, default=24)
    parser.add_argument("--fee-bps", type=float, default=5.0)
    parser.add_argument("--slippage-bps", type=float, default=1.0)
    parser.add_argument("--temperature", type=float, default=2.0)
    parser.add_argument("--max-bars", type=int, default=None)
    args = parser.parse_args()

    frame = load_data(Path(args.data))
    if args.max_bars:
        frame = frame.tail(args.max_bars).copy()

    features = frame[["log_return", "realized_vol_24"]].dropna()
    if len(features) <= args.window + args.horizon:
        raise SystemExit("not enough bars for the requested window")

    print(
        f"bars={len(frame)} features={len(features)} window={args.window} "
        f"refit_every={args.refit_every} horizon={args.horizon}",
        file=sys.stderr,
    )

    garch_vol = rolling_garch(
        features["log_return"] * 100.0,
        args.window,
        args.refit_every,
        args.horizon,
    )
    regimes = rolling_hmm(features, args.window, args.refit_every)

    cost_per_side = (args.fee_bps + args.slippage_bps) / 10_000.0
    cost_round_trip = 2.0 * cost_per_side

    work = build_signals(
        frame,
        garch_vol,
        regimes,
        args.horizon,
        cost_round_trip,
        args.temperature,
    )
    work = work.dropna(
        subset=[
            "garch_vol",
            "p_down",
            "p_range",
            "p_up",
            "mom_72",
            "p_long",
            "p_short",
            "p_no_action",
            "fwd_ret_24",
        ]
    )
    if work.empty:
        raise SystemExit("no valid rows after warm-up")

    positions = pd.Series(
        apply_holding_period(work["signal"].to_numpy(), args.horizon),
        index=work.index,
    )
    turnover = positions.diff().abs().fillna(positions.abs())
    strategy_returns = positions.shift(1).fillna(0) * work["log_return"] - turnover * cost_per_side
    buy_hold = work["log_return"]

    strategy_metrics = metrics(strategy_returns, positions, turnover)
    strategy_metrics["total_cost"] = float((turnover * cost_per_side).sum())
    buy_hold_metrics = metrics(buy_hold, pd.Series(1.0, index=work.index), pd.Series(0.0, index=work.index))

    active = positions.shift(1).fillna(0) != 0
    teacher_edge = strategy_returns[active].dropna()
    summary = {
        "args": vars(args),
        "cost_per_side_bps": args.fee_bps + args.slippage_bps,
        "rows": int(len(work)),
        "signal_counts": {str(k): int(v) for k, v in work["signal"].value_counts().sort_index().items()},
        "strategy": strategy_metrics,
        "buy_hold": buy_hold_metrics,
        "teacher_mean_active_log_return": float(teacher_edge.mean()) if not teacher_edge.empty else None,
        "teacher_active_bars": int(active.sum()),
    }

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    work[
        [
            "close",
            "garch_vol",
            "p_down",
            "p_range",
            "p_up",
            "p_long",
            "p_short",
            "p_no_action",
            "signal",
            "fwd_ret_24",
        ]
    ].to_csv(out)
    summary["out"] = str(out)
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
