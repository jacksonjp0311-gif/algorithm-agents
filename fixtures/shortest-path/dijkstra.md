TITLE: Dijkstra's shortest path
AUTHORS:
- Edsger W. Dijkstra
YEAR: 1959
TYPE: paper
URL: https://doi.org/10.1007/BF01386390
REPOSITORY:
LICENSE: source-described public scientific result
DOMAIN: Graph Algorithms
TAGS:
- shortest path
- weighted graphs
- routing
ABSTRACT: A method for finding shortest paths from a single source in a directed graph whose edge weights are non-negative. The algorithm repeatedly selects the unsettled vertex with the smallest tentative distance and relaxes its outgoing edges.
SECTION_METHOD: Maintain tentative distances d[v], initially 0 at the source and infinity elsewhere. Extract the unused vertex u of least d[u]. For each edge u -> v of weight w, set d[v] = min(d[v], d[u] + w).
EQUATION:
d[v] = min(d[v], d[u] + w(u,v))
VARIABLES:
- d[v]: tentative distance from the source to v
- u: extracted vertex of least tentative distance
- w(u,v): non-negative length of edge u -> v
ASSUMPTIONS:
- All edge weights are non-negative
- The graph is finite
CONSTRAINTS:
- Negative edges invalidate the extract-min argument
COMPLEXITY_TIME: O((V + E) log V) with a binary heap
COMPLEXITY_SPACE: O(V)
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
d[source] = 0
for v != source: d[v] = inf
Q = all vertices
while Q is not empty:
    u = extract_min(Q)
    for each edge u -> v:
        if d[v] > d[u] + w(u,v):
            d[v] = d[u] + w(u,v)
REFERENCE_CODE:
def dijkstra(graph, source):
    import heapq
    dist = {source: 0}
    heap = [(0, source)]
    seen = set()
    while heap:
        d, u = heapq.heappop(heap)
        if u in seen:
            continue
        seen.add(u)
        for v, w in graph.get(u, []):
            nd = d + w
            if nd < dist.get(v, float("inf")):
                dist[v] = nd
                heapq.heappush(heap, (nd, v))
    return dist
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Network routing
- Map directions
POTENTIAL_USES:
- Any non-negative weighted graph search
ALGORITHM: yes
AMBIGUOUS: no
FAILURES:
- Negative edge weights
- Disconnected goals remain at infinity
CITATIONS:
- fixture://shortest-path/bellman-ford.md
MOTIFS:
- select → route → aggregate
