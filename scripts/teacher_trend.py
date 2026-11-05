"""Simpler trend teacher for comparison with the GARCH/HMM version."""

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd


def load_data(path: Path) -> pd.DataFrame:
    frame = pd.read_csv(path)
    frame["open_time"] = pd.to_datetime(frame["open_time"], unit="ms", utc=True)
    frame = frame.sort_values("open_time").set_index("open_time")
    for column in ("open", "high", "low", "close", "volume"):
        frame[column] = pd.to_numeric(frame[column], errors="coerce")
    frame["log_return"] = np.log(frame["close"]).diff()
    frame["realized_vol_24"] = frame["log_return"].rolling(24).std() * math.sqrt(24)
    return frame.dropna(subset=["log_return", "realized_vol_24"])


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


def metrics(returns: pd.Series) -> dict:
    clean = returns.dropna()
    if clean.empty:
        return {}
    equity = np.exp(clean.cumsum())
    drawdown = equity / equity.cummax() - 1.0
    return {
        "bars": int(len(clean)),
        "total_return": float(np.expm1(clean.sum())),
        "annualized_sharpe": float(clean.mean() / clean.std() * math.sqrt(24 * 365))
        if clean.std() > 0
        else 0.0,
        "max_drawdown": float(drawdown.min()),
        "bar_hit_rate": float((clean > 0).mean()),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", default="data/btcusdt_1h.csv")
    parser.add_argument("--out", default="data/teacher_trend.csv")
    parser.add_argument("--max-bars", type=int, default=None)
    parser.add_argument("--fee-bps", type=float, default=5.0)
    parser.add_argument("--slippage-bps", type=float, default=1.0)
    args = parser.parse_args()

    frame = load_data(Path(args.data))
    if args.max_bars:
        frame = frame.tail(args.max_bars).copy()

    cost_per_side = (args.fee_bps + args.slippage_bps) / 10_000.0
    cost_round_trip = 2.0 * cost_per_side

    frame["mom_24"] = frame["close"].pct_change(24)
    frame["mom_168"] = frame["close"].pct_change(168)
    frame["vol"] = frame["realized_vol_24"]
    vol_cutoff = frame["vol"].quantile(0.90)

    signal = np.zeros(len(frame))
    for i in range(len(frame)):
        m24 = frame["mom_24"].iloc[i]
        m168 = frame["mom_168"].iloc[i]
        vol = frame["vol"].iloc[i]
        if not (math.isfinite(m24) and math.isfinite(m168) and math.isfinite(vol)):
            continue
        if vol > vol_cutoff:
            continue
        if m24 > 0 and m168 > 0:
            signal[i] = 1
        elif m24 < 0 and m168 < 0:
            signal[i] = -1

    positions = pd.Series(
        apply_holding_period(signal, 48), index=frame.index
    )
    turnover = positions.diff().abs().fillna(positions.abs())
    strategy_returns = positions.shift(1).fillna(0) * frame["log_return"] - turnover * cost_per_side
    buy_hold = frame["log_return"]

    summary = {
        "strategy": metrics(strategy_returns),
        "buy_hold": metrics(buy_hold),
        "signal_counts": {str(int(k)): int(v) for k, v in pd.Series(signal).value_counts().sort_index().items()},
        "position_changes": int((turnover > 0).sum()),
        "total_cost": float((turnover * cost_per_side).sum()),
    }
    print(json.dumps(summary, indent=2, sort_keys=True))

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    pd.DataFrame({"signal": signal, "position": positions}, index=frame.index).to_csv(out)


if __name__ == "__main__":
    main()
