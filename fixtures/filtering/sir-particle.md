TITLE: Sequential importance resampling
AUTHORS:
- N. J. Gordon
- D. J. Salmond
- A. F. M. Smith
YEAR: 1993
TYPE: paper
URL: https://doi.org/10.1049/ip-f-2.1993.0015
DOMAIN: State Estimation
TAGS:
- particle filter
- sequential monte carlo
- resampling
ABSTRACT: Represent a filtering distribution by weighted samples. At each time, propagate particles through the dynamics, reweight by the measurement likelihood, then resample with replacement according to the weights so that particles concentrate where the posterior is large. The 1993 IEE paper is the bootstrap / SIR filter for nonlinear non-Gaussian tracking.
EQUATION:
w_t^(i) proportional to w_{t-1}^(i) * p(z_t | x_t^(i))
then resample x from the weighted cloud
VARIABLES:
- x^(i): particle state
- w^(i): importance weight
- z: measurement
ASSUMPTIONS:
- A generative state-space model with evaluable likelihood
- Enough particles to cover the posterior
CONSTRAINTS:
- Degeneracy without resampling
- Sample impoverishment after aggressive resampling
COMPLEXITY_TIME: O(N) per time plus resampling
COMPLEXITY_SPACE: O(N)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
draw x0 from prior
for each time t:
    for each particle i:
        x_t^(i) ~ p(x_t | x_{t-1}^(i))
        w_t^(i) = w_{t-1}^(i) * p(z_t | x_t^(i))
    normalize weights
    resample N particles with replacement
REFERENCE_CODE:
import random
def sir_step(particles, weights, evolve, likelihood, z):
    particles = [evolve(x) for x in particles]
    weights = [w * likelihood(x, z) for x, w in zip(particles, weights)]
    s = sum(weights) or 1.0
    weights = [w / s for w in weights]
    cdf = []
    acc = 0.0
    for w in weights:
        acc += w
        cdf.append(acc)
    out = []
    for _ in particles:
        u = random.random()
        j = next(i for i, c in enumerate(cdf) if c >= u)
        out.append(particles[j])
    n = len(out)
    return out, [1.0 / n] * n
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Radar / sonar tracking
- Nonlinear navigation
ALGORITHM: yes

---EMERGENT---
TITLE: Effective-sample-size trigger
AUTHORS:
- N. J. Gordon
- D. J. Salmond
- A. F. M. Smith
YEAR: 1993
TYPE: note
URL: https://doi.org/10.1049/ip-f-2.1993.0015
DOMAIN: State Estimation
TAGS:
- effective sample size
- degeneracy
ABSTRACT: The SIR paper is about resampling every step. A later, widely used companion test — not the paper's title algorithm — decides *when* to resample: compute N_eff = 1 / sum_i w_i^2 and resample only if N_eff falls below a threshold. That trigger is a distinct computational procedure that rides along particle-filter practice.
EQUATION:
N_eff = 1 / sum_i (w_i)^2
resample iff N_eff < N_threshold
VARIABLES:
- w_i: normalized weights
- N_eff: effective sample size
ASSUMPTIONS:
- Weights are nonnegative and sum to one
COMPLEXITY_TIME: O(N)
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
neff = 1 / sum(w*w for w in weights)
if neff < 0.5 * N: resample
ALGORITHM: yes
KNOWN_USES:
- Adaptive resampling in particle filters
POTENTIAL_USES:
- Any weighted ensemble that must detect collapse
