# Roster

Twenty-six contracts. Not personalities. You do not chat with them. You dispatch them, or `algo do` does.

They are grouped the way the work actually moves.

## Acquisition — they find and bring

| id | Duty |
| --- | --- |
| `source_scout` | Rank fixtures and `sources/` against the objective. May receive extra URLs. |
| `web_collector` | Retrieve a locator. Live pages are rewritten to clean APIs when possible. |
| `paper_analyst` | Lift title, sections, equations, citations from a document. |
| `repository_scout` | Record repository metadata from a supplied source. |
| `citation_walker` | Follow labeled citations within session budget. |

## Analysis — they ask if a procedure is there

| id | Duty |
| --- | --- |
| `algorithm_detector` | YES / NO / UNCERTAIN. The gate. |
| `algorithm_extractor` | Normalize fields into an extraction candidate. May request math. |
| `math_analyst` | Equations, variables, recurrences, constraints. |
| `code_analyst` | Structure of supplied implementation only. |
| `complexity_analyst` | Time / space and whether that claim is labeled or derived. |
| `assumption_analyst` | Assumptions the source actually stated. |
| `failure_mode_analyst` | Edge cases and known breaks. |

## Verification — they are allowed to ruin a good story

| id | Duty |
| --- | --- |
| `provenance_checker` | Can this be tied to a source, or is it floating? |
| `cross_source_verifier` | Does another permitted source agree? |
| `math_checker` | Challenge the math reading. |
| `code_verifier` | Sandboxed `python -I` on a snippet, or `NOT_TESTED`. |
| `hallucination_challenger` | Attack claims that are not in the source. |

## Knowledge — they place the candidate among what you already keep

| id | Duty |
| --- | --- |
| `deduplicator` | Block or flag what the archive already holds. |
| `relationship_mapper` | Neighbor procedures. |
| `structural_matcher` | Same shape, different name. |
| `domain_classifier` | Where it sits. |
| `use_case_mapper` | Known and possible uses from the source. |

## Synthesis — they shape, they do not bless

| id | Duty |
| --- | --- |
| `normalizer` | One shape of candidate. |
| `summarizer` | Short reading of what was extracted. |
| `code_translator` | Generated reference code, labeled as generated. |
| `experiment_designer` | A test one could run. Not a proof. |

## How they speak

Messages are typed. There is no free-form peer chat.

`ANALYSIS_REQUEST`, `ANALYSIS_RESULT`, `VERIFICATION_REQUEST`, `VERIFICATION_RESULT`, `CLARIFICATION_REQUEST`, `CLARIFICATION_RESULT`, `ARTIFACT_REFERENCE`, `BLOCKED_NOTICE`, `LOW_CONFIDENCE_NOTICE`.

The extractor may ask the math analyst. That is a wire, not a conversation.

Contracts live in `agents/<id>/`. The enabled list is `registry/agents.json`. If an id is not there, it is not on the crew.
