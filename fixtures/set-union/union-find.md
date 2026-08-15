TITLE: Disjoint-set union with path compression
AUTHORS:
- Robert Endre Tarjan
YEAR: 1975
TYPE: paper
URL: https://doi.org/10.1145/321879.321884
DOMAIN: Data Structures
TAGS:
- union-find
- disjoint sets
- path compression
ABSTRACT: Maintain a partition of a finite set under Union and Find. Represent each class by a rooted tree. Find follows parent pointers to a root. Path compression flattens the nodes visited by a Find onto that root, so later Finds on the same chain are cheap.
SECTION_METHOD: Each element points to a parent. A Find walks to the root, then rewires every visited node to the root. A Union links two roots.
EQUATION:
Find(x) = root(x) after parent[y] := root for every y on the path
VARIABLES:
- parent[x]: next node toward the class representative
- root: representative of a class
ASSUMPTIONS:
- The universe is finite
- Unions only link current roots
CONSTRAINTS:
- Without a balanced linking rule, trees can still be deep before compression
COMPLEXITY_TIME: Almost inverse-Ackermann per operation when paired with union-by-rank
COMPLEXITY_SPACE: O(n)
COMPLEXITY_ORIGIN: SOURCE_STATED
PSEUDOCODE:
function find(x):
    if parent[x] != x:
        parent[x] = find(parent[x])
    return parent[x]
function union(a, b):
    ra = find(a); rb = find(b)
    if ra != rb: parent[ra] = rb
REFERENCE_CODE:
def find(parent, x):
    if parent[x] != x:
        parent[x] = find(parent, parent[x])
    return parent[x]
REFERENCE_LANGUAGE: python
KNOWN_USES:
- Kruskal minimum spanning tree
- Connected components
POTENTIAL_USES:
- Incremental equivalence tracking
ALGORITHM: yes
CITATIONS:
- fixture://set-union/union-find.md

---EMERGENT---
TITLE: Union by rank
AUTHORS:
- Robert Endre Tarjan
YEAR: 1975
TYPE: paper
URL: https://doi.org/10.1145/321879.321884
DOMAIN: Data Structures
TAGS:
- union-by-rank
- balanced linking
ABSTRACT: The same disjoint-set paper (and the Hopcroft–Ullman / Tarjan linking analysis) carries a second procedure: when two roots are united, hang the shallower tree under the deeper one using a rank or size field. This is not the named Find algorithm; it is the linking rule that keeps trees shallow so path compression stays cheap.
EQUATION:
if rank[a] < rank[b]: parent[a] = b else parent[b] = a; if equal, rank[a] += 1
VARIABLES:
- rank[x]: upper bound on tree height
ASSUMPTIONS:
- Ranks change only when two equal-rank roots are linked
COMPLEXITY_TIME: Combined with path compression, amortized inverse-Ackermann
COMPLEXITY_SPACE: O(n)
COMPLEXITY_ORIGIN: DERIVED
PSEUDOCODE:
ra = find(a); rb = find(b)
if rank[ra] < rank[rb]: parent[ra] = rb
else:
    parent[rb] = ra
    if rank[ra] == rank[rb]: rank[ra] += 1
ALGORITHM: yes
KNOWN_USES:
- Balanced linking inside Union-Find
POTENTIAL_USES:
- Any forest that must stay shallow under merges
