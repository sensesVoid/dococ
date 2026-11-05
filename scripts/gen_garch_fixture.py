import json, math
import numpy as np
import pandas as pd
from arch import arch_model

rng = np.random.default_rng(12345)
n = 60
rets = rng.normal(0, 1, n) * 0.5  # percent-scale-ish returns
series = pd.Series(rets)

model = arch_model(series, mean="Zero", vol="Garch", p=1, q=1, dist="normal", rescale=False)
res = model.fit(disp="off", show_warning=False)
params = {
    "omega": float(res.params["omega"]),
    "alpha": float(res.params["alpha[1]"]),
    "beta": float(res.params["beta[1]"]),
}
cv = res.conditional_volatility.to_numpy(dtype=float).tolist()

fixture = {
    "returns": rets.tolist(),
    "params": params,
    "expected_vol": cv,
}
out = "crates/dococ-quant/tests/garch_fixture.json"
import os
os.makedirs("crates/dococ-quant/tests", exist_ok=True)
with open(out, "w") as f:
    json.dump(fixture, f)
print("wrote", out)
print("params", params)
print("n", len(rets))
