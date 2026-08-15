TITLE: Bellman-Ford shortest path
AUTHORS:
- Richard Bellman
YEAR: 1958
TYPE: paper
URL: https://doi.org/10.1090/qam/102435
DOMAIN: Graph Algorithms
TAGS:
- shortest path
- negative edges
ABSTRACT: A single-source shortest-path method that relaxes every edge |V|-1 times and can detect negative cycles.
EQUATION:
d[v] = min(d[v], d[u] + w(u,v))
ASSUMPTIONS:
- Graph is finite
CONSTRAINTS:
- Negative cycles make a shortest path undefined
COMPLEXITY_TIME: O(V E)
COMPLEXITY_SPACE: O(V)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
repeat |V|-1 times:
    relax every edge
check once more for a negative cycle
ALGORITHM: yes
CITATIONS:
- fixture://shortest-path/dijkstra.md
