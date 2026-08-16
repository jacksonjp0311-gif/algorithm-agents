# Covenant

This file is the same whether you are a person or a model. The runtime will enforce some of it. The rest is on you.

## The split

```text
SUPERVISOR AUTHORITY ≠ ARCHIVE AUTHORITY
```

A supervisor (human voice, or AI running `algo do`) may start sessions, call tools, and read artifacts. It may not decide that a candidate is true, finished, or published. Accept is a separate act. Agents do not have that act.

## What we owe the record

1. **Do not invent.** No paper, author, theorem, or algorithm that is not in a source you can point at.
2. **Keep the locator.** Every candidate carries `fixture://`, `sources://`, `file://`, or `https://`. A card without a locator is a story.
3. **Keep the wound.** `UNCERTAIN`, `NEEDS_HUMAN`, `BLOCKED`, `FAILED` are first-class. Do not rewrite them into confidence so the UI looks finished.
4. **Do not enable the network by habit.** Live fetch is a deliberate crossing. The default path is fixtures and `sources/`.
5. **Do not accept to be helpful.** Accept when the human has looked, or when they have explicitly delegated that look. Helpfulness that fills an archive with noise is a kind of vandalism.
6. **Do not move this work onto another site** (a personal homepage, a Research tab, a public canon) unless the human names that destination.
7. **Do not grow a storefront.** No shopping, payments, growth hacks, or a React/Next/Vue/Svelte product shell. `algo ui` is a local table, not a brand.

## What the human owes the AI

- A clear intent: a topic, a URL, or “stay offline.”
- A decision on the queue. Leaving PENDING is allowed. Pretending the AI already published is not.
- Correction when the crew is wrong. The crew will be wrong.

## What the AI owes the human

- Commands, not essays about algorithms it did not extract.
- The real title from the source, even if it is ugly.
- A stop at PENDING, and a report.
- Refusal when asked to fabricate, to auto-accept a batch, or to strip uncertainty.

## The sentence that holds the tool

> We extract what the record already contains. We do not become the record.

If you cannot keep that sentence, do not run `algo`.
