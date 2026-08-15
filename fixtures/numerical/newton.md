TITLE: Newton-Raphson iteration
AUTHORS:
- Isaac Newton
- Joseph Raphson
YEAR: 1690
TYPE: book
URL: https://mathworld.wolfram.com/NewtonsMethod.html
DOMAIN: Numerical Methods
TAGS:
- newton
- root finding
- second-order
ABSTRACT: Find a root of a differentiable map by replacing it with its local linear model and solving that model. The named Newton-Raphson step is x ← x - f(x)/f'(x) in one dimension, or the analogous linear solve in several. The Jacobian construction is a companion procedure, not the name of the method.
EQUATION:
x_{k+1} = x_k - J(x_k)^{-1} f(x_k)
VARIABLES:
- f: residual
- J: Jacobian of f
- x: current guess
ASSUMPTIONS:
- J is invertible near the root
- The initial guess is close enough for the quadratic model to dominate
CONSTRAINTS:
- Singular or ill-conditioned J breaks the step
COMPLEXITY_TIME: A linear solve of size n per iteration
COMPLEXITY_SPACE: O(n^2) if J is stored densely
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
while not converged:
    J = jacobian(f, x)
    solve J d = f(x)
    x = x - d
REFERENCE_CODE:
def newton_scalar(f, df, x, steps=20):
    for _ in range(steps):
        d = df(x)
        if d == 0:
            return x
        x = x - f(x) / d
    return x
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Root finding
- Local nonlinear solves
ALGORITHM: yes

---EMERGENT---
TITLE: Jacobian residual linearization
AUTHORS:
- Isaac Newton
- Joseph Raphson
YEAR: 1690
TYPE: notes
URL: https://mathworld.wolfram.com/NewtonsMethod.html
DOMAIN: Numerical Methods
TAGS:
- jacobian
- linearization
ABSTRACT: Inside Newton sits a second computational object: build the local linear map J and treat f(x) + J d ≈ 0 as the problem to solve. That linearization is reusable without committing to a full Newton loop — for example in Gauss-Newton or in a single corrective step.
EQUATION:
f(x + d) ≈ f(x) + J(x) d
VARIABLES:
- J: derivative map
- d: increment
ASSUMPTIONS:
- f is differentiable at x
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
form J at x
treat the increment as the solve of J d = -f(x)
ALGORITHM: yes
KNOWN_USES:
- Local models for nonlinear least squares
POTENTIAL_USES:
- Any corrector that needs a first-order residual map
