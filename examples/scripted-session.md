# Scripted supervisor example

```text
algo session create --objective "Find shortest-path algorithms" --supervisor scripted
algo tool search_sources --args "{\"session_id\":\"RS-0001\",\"input\":{\"objective\":\"weighted shortest path\"}}"
algo tool retrieve_source --args "{\"session_id\":\"RS-0001\",\"locator\":\"fixture://shortest-path/dijkstra.md\"}"
algo run algorithm_extractor --session RS-0001 --artifact ART-0001
algo smoke
```

Or the one-shot path:

```text
algo harvest "weighted shortest path"
algo archive list
algo archive accept CAND-0001
```

`submit_review_candidate` creates a PENDING extraction only.
