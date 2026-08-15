TITLE: Viterbi decoding
AUTHORS:
- Andrew J. Viterbi
YEAR: 1967
TYPE: paper
URL: https://doi.org/10.1109/TIT.1967.1054010
DOMAIN: Sequence Models
TAGS:
- hidden markov
- dynamic programming
- decoding
ABSTRACT: Find the most likely hidden state sequence for a convolutional code (and, by the same recursion, a hidden Markov model). At each time and state, keep only the best predecessor path and its score. The 1967 IEEE Transactions on Information Theory paper gives the error-bound and the asymptotically optimum decoder.
EQUATION:
delta_t(j) = max_i [ delta_{t-1}(i) + log a_{ij} ] + log b_j(o_t)
VARIABLES:
- delta_t(j): best path score into state j at time t
- a_ij: transition from i to j
- b_j(o): emission of observation o in state j
ASSUMPTIONS:
- First-order Markov hidden chain
- Finite state set
CONSTRAINTS:
- Continuous state spaces need a different decoder
COMPLEXITY_TIME: O(T |S|^2)
COMPLEXITY_SPACE: O(T |S|)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
initialize delta_1 from start * emit
for t = 2..T:
    for each state j:
        delta_t(j) = max over i of delta_{t-1}(i) + trans(i,j) + emit(j, o_t)
        psi_t(j) = argmax predecessor
backtrace from the last best state
REFERENCE_CODE:
def viterbi(obs, states, start, trans, emit):
    delta = [{s: start[s] + emit[s][obs[0]] for s in states}]
    psi = [{}]
    for t in range(1, len(obs)):
        d = {}
        p = {}
        for j in states:
            i_best = max(states, key=lambda i: delta[-1][i] + trans[i][j])
            d[j] = delta[-1][i_best] + trans[i_best][j] + emit[j][obs[t]]
            p[j] = i_best
        delta.append(d)
        psi.append(p)
    last = max(states, key=lambda s: delta[-1][s])
    path = [last]
    for t in range(len(obs) - 1, 0, -1):
        last = psi[t][last]
        path.append(last)
    path.reverse()
    return path
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Convolutional decoding
- Speech / HMM alignment
ALGORITHM: yes

---EMERGENT---
TITLE: Trellis max-product
AUTHORS:
- Andrew J. Viterbi
YEAR: 1967
TYPE: paper
URL: https://doi.org/10.1109/TIT.1967.1054010
DOMAIN: Sequence Models
TAGS:
- max-product
- trellis
- dynamic programming
ABSTRACT: The named subject is a decoder. The same recurrence is a max-product message pass on a chain-shaped factor graph: at each node keep only the best incoming product and discard the rest. That structural motif is not the paper's title; it is the computational skeleton later reused in belief-propagation style decoders.
EQUATION:
m_t(j) = max_i m_{t-1}(i) * psi(i, j, o_t)
VARIABLES:
- m_t: incoming max-product message
- psi: local trellis factor
ASSUMPTIONS:
- The graph is a chain / trellis
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
for each time and state:
    keep only the best incoming product
ALGORITHM: yes
KNOWN_USES:
- Max-product on chains
POTENTIAL_USES:
- Other trellis search problems with a local score
