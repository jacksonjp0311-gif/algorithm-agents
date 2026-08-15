TITLE: Metropolis-Hastings sampling
AUTHORS:
- W. K. Hastings
YEAR: 1970
TYPE: paper
URL: https://doi.org/10.1093/biomet/57.1.97
DOMAIN: Monte Carlo
TAGS:
- mcmc
- sampling
- markov chain
ABSTRACT: Construct a Markov chain whose invariant distribution is a target density p that is known only up to a constant. From state x, propose y ~ q(y|x). Accept the proposal with probability alpha, otherwise stay at x. The 1970 Biometrika paper generalizes the Metropolis rule to asymmetric proposals.
EQUATION:
alpha(x, y) = min(1, [p(y) q(x|y)] / [p(x) q(y|x)])
VARIABLES:
- p: target density up to a constant
- q: proposal kernel
- alpha: acceptance probability
ASSUMPTIONS:
- The chain is irreducible and aperiodic on the support of p
- p(x) > 0 on the region of interest
CONSTRAINTS:
- Poor proposals mix slowly
COMPLEXITY_TIME: One proposal plus one density-ratio evaluation per step
COMPLEXITY_SPACE: O(state dimension)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
x = x0
for t in 1..T:
    y ~ q(.|x)
    u ~ Uniform(0,1)
    if u < alpha(x, y): x = y
    emit x
REFERENCE_CODE:
import random
def mh_step(x, log_p, propose, log_q):
    y = propose(x)
    log_a = log_p(y) + log_q(y, x) - log_p(x) - log_q(x, y)
    if random.random() < min(1.0, pow(2.718281828, log_a)):
        return y
    return x
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Bayesian posterior sampling
- Statistical physics
ALGORITHM: yes

---EMERGENT---
TITLE: Detailed-balance acceptance
AUTHORS:
- W. K. Hastings
YEAR: 1970
TYPE: paper
URL: https://doi.org/10.1093/biomet/57.1.97
DOMAIN: Monte Carlo
TAGS:
- detailed balance
- reversibility
ABSTRACT: Inside the same Hastings construction sits a more general computational object: choose any acceptance function that restores detailed balance p(x) q(y|x) alpha(x,y) = p(y) q(x|y) alpha(y,x). The named Metropolis-Hastings step is one solution. The balance identity itself is the latent design rule and can generate other valid kernels.
EQUATION:
p(x) q(y|x) alpha(x,y) = p(y) q(x|y) alpha(y,x)
VARIABLES:
- alpha: any [0,1]-valued acceptance that satisfies balance
ASSUMPTIONS:
- Positive proposal mass on a reversible pair of states
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
choose alpha so the probability flux x->y equals y->x
ALGORITHM: yes
KNOWN_USES:
- Designing reversible MCMC kernels
POTENTIAL_USES:
- Other accept/reject correctors that must preserve a target measure
