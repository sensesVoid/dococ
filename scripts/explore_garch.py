import json, math
import numpy as np
import pandas as pd
from arch import arch_model

rng = np.random.default_rng(42)
n = 200
rets = rng.normal(0, 1, n) * 0.5  # percent-scale returns
series = pd.Series(rets * 100.0)  # arch expects percent returns typically; here just raw*100

model = arch_model(series, mean="Zero", vol="Garch", p=1, q=1, dist="normal", rescale=False)
res = model.fit(disp="off", show_warning=False)
print("params:", res.params.to_dict())
cv = res.conditional_volatility
print("cv head:", cv.head(8).tolist())
print("cv len:", len(cv))

# naive recursion with arch params
omega = float(res.params["omega"])
alpha = float(res.params["alpha[1]"])
beta = float(res.params["beta[1]"])
print("omega,alpha,beta =", omega, alpha, beta)
# arch conditional_volatility is std (sqrt variance). Convert returns to variance-scale?
# arch works on the input series (percent). residual = return - mean (0). variance recursion on squared residuals.
r = series.to_numpy(dtype=float)
var = np.empty(n)
# init: arch uses sample variance? Let's try unconditional
uncond = omega / (1 - alpha - beta)
var[0] = uncond
for t in range(1, n):
    var[t] = omega + alpha * (r[t-1]**2) + beta * var[t-1]
vol = np.sqrt(var)
print("naive vol head:", vol[:8].tolist())
print("max abs diff:", np.max(np.abs(vol - cv.to_numpy())))
