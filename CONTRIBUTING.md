# Contributing to Alchetron

Preserve the authority split: models and agents may create private artifacts, but only explicit human review may move canon.

Every change should include the smallest relevant regression case. New extraction behavior belongs in the evaluation corpus. New graph relations require ontology documentation and integrity coverage. New model adapters must never persist credentials or receive archive authority.

Run `./scripts/verify.ps1` on Windows or the equivalent Cargo and CLI checks from CI before submitting work.
