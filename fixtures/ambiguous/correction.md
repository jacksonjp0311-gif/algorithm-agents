TITLE: Ambiguous correction term
AUTHORS:
- Unknown fragment
YEAR: 1970
TYPE: note
DOMAIN: State Estimation
ABSTRACT: A short fragment that describes a correction to an estimate but writes two incompatible signs for the same update and never chooses between them.
EQUATION:
x' = x + k * e
x' = x - k * e
VARIABLES:
- x: estimate
- k: gain
- e: residual
ASSUMPTIONS:
- A residual is available
AMBIGUOUS: yes
AMBIGUITY: Two plausible interpretations of the correction sign are present in the same fragment. Equation 1 adds the residual; equation 2 subtracts it. The source does not say which is intended.
ALGORITHM: yes
PSEUDOCODE:
update the estimate by some signed multiple of the residual
FAILURES:
- Wrong sign inverts the correction and can diverge
COMPLEXITY_ORIGIN: UNKNOWN
