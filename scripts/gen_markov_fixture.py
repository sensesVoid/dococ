import json, os
import numpy as np
from hmmlearn.hmm import GaussianHMM

rng = np.random.default_rng(99)
n = 40
obs = np.concatenate([rng.normal(-1, 0.5, n//2), rng.normal(1, 0.5, n//2)]).reshape(-1, 1)
model = GaussianHMM(n_components=2, covariance_type="diag", n_iter=50, tol=1e-3, random_state=42)
model.fit(obs)
order = np.argsort(model.means_[:, 0])
trans = model.transmat_[order][:, order]
means = model.means_[order]
covars = np.asarray(model.covars_[order]).reshape(2, -1)
start = model.startprob_[order]

K = 2
gamma = np.zeros((n, K))
for t in range(n):
    if t == 0:
        pred = start.copy()
    else:
        pred = gamma[t-1] @ trans
    lik = np.zeros(K)
    for s in range(K):
        c = float(covars[s, 0])
        lik[s] = np.exp(-0.5*((obs[t,0]-means[s,0])**2)/c - 0.5*np.log(c) - 0.5*np.log(2*np.pi)) * pred[s]
    ssum = lik.sum()
    gamma[t] = lik / ssum if ssum > 0 else np.ones(K)/K

fixture = {
    "n_states": K,
    "transition": trans.tolist(),
    "means": means.tolist(),
    "variances": covars.tolist(),
    "initial": start.tolist(),
    "obs": obs.flatten().tolist(),
    "expected": gamma.tolist(),
}
os.makedirs("crates/dococ-quant/tests", exist_ok=True)
with open("crates/dococ-quant/tests/markov_fixture.json", "w") as f:
    json.dump(fixture, f)
print("done")
