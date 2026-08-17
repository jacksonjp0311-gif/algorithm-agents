# Evaluation

`algo eval run` executes the versioned corpus in `eval/corpus.json` and persists a report. The release gate requires perfect fixture title, domain, algorithm-presence, and exact evidence-claim coverage on the locked corpus.

This is a regression baseline, not a claim of general mathematical correctness. Add difficult cases, counterexamples, ambiguous sources, and adversarial formatting as the runtime grows. Never weaken a threshold merely to make CI green without documenting why.
