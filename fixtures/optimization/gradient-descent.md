TITLE: Gradient descent
AUTHORS:
- Augustin-Louis Cauchy
YEAR: 1847
TYPE: paper
URL: https://en.wikipedia.org/wiki/Gradient_descent
DOMAIN: Optimization
TAGS:
- gradient descent
- steepest descent
- first-order
ABSTRACT: Minimize a differentiable objective by repeatedly stepping opposite the gradient. Cauchy's 1847 steepest-descent idea is the named procedure: from x, compute ∇f(x) and move downhill. It is a first-order method. It does not name the line-search that later practice hangs on the same step.
EQUATION:
x_{k+1} = x_k - α_k ∇f(x_k)
VARIABLES:
- x: iterate
- α: step size
- ∇f: gradient of the objective
ASSUMPTIONS:
- f is differentiable on the path of iterates
- A step size exists that decreases f
CONSTRAINTS:
- Ill-conditioned Hessians make plain steps crawl
COMPLEXITY_TIME: One gradient per iteration
COMPLEXITY_SPACE: O(dimension)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
x = x0
while not converged:
    g = grad(f, x)
    x = x - alpha * g
REFERENCE_CODE:
def gradient_descent(x, grad, alpha=0.05, steps=100):
    for _ in range(steps):
        g = grad(x)
        x = [xi - alpha * gi for xi, gi in zip(x, g)]
    return x
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Unconstrained smooth minimization
- Training first-order models
ALGORITHM: yes

---EMERGENT---
TITLE: Backtracking line search
AUTHORS:
- Larry Armijo
YEAR: 1966
TYPE: paper
URL: https://doi.org/10.2140/pjm.1966.16.1
DOMAIN: Optimization
TAGS:
- line search
- Armijo
- step size
ABSTRACT: The named subject is the descent step. A second procedure chooses α: start from a trial step and shrink it until f(x - αg) satisfies a sufficient-decrease test. That is not Cauchy's algorithm. It is the control law that makes the same gradient usable.
EQUATION:
accept α if f(x - α g) ≤ f(x) - c α ||g||^2
VARIABLES:
- α: trial step
- c: sufficient-decrease constant in (0,1)
ASSUMPTIONS:
- f is bounded below along the ray
COMPLEXITY_TIME: A few extra function evaluations per iteration
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
alpha = 1
while f(x - alpha * g) > f(x) - c * alpha * dot(g,g):
    alpha = beta * alpha
ALGORITHM: yes
KNOWN_USES:
- Globalization of gradient and Newton steps
POTENTIAL_USES:
- Any first-order update that must refuse an overshoot
