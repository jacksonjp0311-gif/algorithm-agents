TITLE: Kalman filter
AUTHORS:
- Rudolf E. Kalman
YEAR: 1960
TYPE: paper
URL: https://doi.org/10.1115/1.3662552
DOMAIN: State Estimation
TAGS:
- filtering
- recursion
- estimation
ABSTRACT: A recursive estimator that predicts a linear Gaussian state, then corrects the prediction with a new measurement. The update is an observe, estimate, correct loop.
EQUATION:
x_hat[k|k] = x_hat[k|k-1] + K[k] (z[k] - H x_hat[k|k-1])
P[k|k] = (I - K[k] H) P[k|k-1]
VARIABLES:
- x_hat: state estimate
- P: error covariance
- K: Kalman gain
- z: measurement
ASSUMPTIONS:
- Linear process and measurement models
- Process and measurement noise are zero-mean Gaussian
CONSTRAINTS:
- Model mismatch degrades the estimate
COMPLEXITY_TIME: cubic in state dimension per update
COMPLEXITY_SPACE: O(n^2)
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
predict state and covariance
compute gain
correct estimate with measurement residual
ALGORITHM: yes
KNOWN_USES:
- Navigation
- Tracking
POTENTIAL_USES:
- Other linear-Gaussian sequential estimation
MOTIFS:
- observe → estimate → correct
FAILURES:
- Unmodeled nonlinearity
- Divergence under wrong noise statistics
CITATIONS:
- fixture://ambiguous/correction.md
