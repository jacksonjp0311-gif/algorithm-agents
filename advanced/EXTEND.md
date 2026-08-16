# Extend

The crew can grow. The split cannot move.

## Add an agent

1. Create `agents/<id>/` with `agent.json`, input/output schemas, smoke fixtures.
2. Register the id in `registry/agents.json`.
3. Implement **one** match arm in `src/agents/`.
4. Run `algo smoke --agent <id>` then `algo smoke --process`.
5. The agent is enabled only if schema validation and smoke pass.

Do not edit the orchestrator to “just add a call.” Do not give the new agent `publish_algorithm`, `approve_extraction`, or unrestricted shell.

## Add a source kind

Local notes go in `sources/` and are already scouted.  
New live hosts must pass the same SSRF rules in `collect.rs`. Empty allowlist means public hosts; private and loopback stay denied.

If you add a cleaner API for a host (the way arXiv abs becomes the export API), put the rewrite in `canonical_fetch_url` and the labeled reconstruction in `normalize_source_text`. The detector should see a document, not a website chrome.

## Add a command

Prefer teaching `algo do` a new intent over adding a twelfth verb. Humans speak sentences. The AI should keep one door.

## What extension is not

- A second archive that auto-accepts
- A web product
- A personality layer on top of the roster
- A way around UNCERTAIN

When you are done, the covenant still reads the same. If it does not, you extended the wrong thing.
