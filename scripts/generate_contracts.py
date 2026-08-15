#!/usr/bin/env python3
"""Generate agent contracts, registries, and schemas."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

AGENTS = [
    ("source_scout", "Source Scout", "acquisition", "Find potentially relevant public technical sources according to the session objective.", ["search_sources", "submit_artifact"], ["math_analyst"]),
    ("web_collector", "Web Collector", "acquisition", "Retrieve permitted public website material for analysis.", ["retrieve_source", "submit_artifact"], []),
    ("paper_analyst", "Paper Analyst", "acquisition", "Inspect supplied papers or research documents without mirroring entire documents.", ["inspect_document", "submit_artifact"], ["algorithm_detector"]),
    ("repository_scout", "Repository Scout", "acquisition", "Inspect public repositories for implementation references relevant to a directive.", ["inspect_repository", "submit_artifact"], []),
    ("citation_walker", "Citation Walker", "acquisition", "Follow relevant citations in a bounded search.", ["search_sources", "submit_artifact"], ["source_scout"]),
    ("algorithm_detector", "Algorithm Detector", "analysis", "Decide whether a source contains a computational procedure worth extracting.", ["detect_algorithm", "submit_artifact"], ["algorithm_extractor"]),
    ("algorithm_extractor", "Algorithm Extractor", "analysis", "Extract explicit or reconstructable computational procedures from supplied technical material.", ["extract_algorithm", "request_math_analysis", "submit_artifact"], ["math_analyst"]),
    ("math_analyst", "Math Analyst", "analysis", "Extract and normalize equations, variables, operators, recurrences, constraints, and update rules.", ["analyze_math", "submit_artifact"], ["math_checker"]),
    ("code_analyst", "Code Analyst", "analysis", "Analyze supplied implementation material for structure, state, and data flow.", ["analyze_code", "submit_artifact"], ["code_verifier"]),
    ("complexity_analyst", "Complexity Analyst", "analysis", "Analyze time and space complexity and mark SOURCE_STATED, DERIVED, or UNKNOWN.", ["analyze_complexity", "submit_artifact"], []),
    ("assumption_analyst", "Assumption Analyst", "analysis", "Extract assumptions required for the method to work.", ["analyze_assumptions", "submit_artifact"], []),
    ("failure_mode_analyst", "Failure-Mode Analyst", "analysis", "Identify edge cases, instability, invalid inputs, and known limitations.", ["analyze_failures", "submit_artifact"], []),
    ("provenance_checker", "Provenance Checker", "verification", "Challenge every candidate claim against its source.", ["verify_provenance", "submit_artifact"], ["hallucination_challenger"]),
    ("cross_source_verifier", "Cross-Source Verifier", "verification", "Check important claims against additional independent sources without manufacturing certainty.", ["verify_provenance", "submit_artifact"], []),
    ("math_checker", "Math Checker", "verification", "Perform bounded symbolic or numerical sanity checking where possible.", ["verify_math", "submit_artifact"], []),
    ("code_verifier", "Code Verifier", "verification", "Execute only explicitly bounded reference implementations.", ["verify_code", "submit_artifact"], []),
    ("hallucination_challenger", "Hallucination Challenger", "verification", "Try to disprove or weaken unsupported extraction claims.", ["challenge_claims", "submit_artifact"], []),
    ("deduplicator", "Deduplicator", "knowledge", "Compare a candidate against the canonical archive.", ["find_duplicates", "submit_artifact"], []),
    ("relationship_mapper", "Relationship Mapper", "knowledge", "Find related algorithms, research, projects, notes, and labs.", ["find_relationships", "submit_artifact"], []),
    ("structural_matcher", "Structural Matcher", "knowledge", "Compare computational structure rather than names alone.", ["find_structural_matches", "submit_artifact"], []),
    ("domain_classifier", "Domain Classifier", "knowledge", "Assign domains and tags.", ["classify_domain", "submit_artifact"], []),
    ("use_case_mapper", "Use-Case Mapper", "knowledge", "Separate known uses from potential uses.", ["map_use_cases", "submit_artifact"], []),
    ("normalizer", "Normalizer", "synthesis", "Convert heterogeneous agent outputs into the canonical archive candidate schema.", ["normalize_candidate", "submit_artifact"], []),
    ("summarizer", "Summarizer", "synthesis", "Produce the human-facing explanation used in compact cards and detailed archive views.", ["summarize_candidate", "submit_artifact"], []),
    ("code_translator", "Code Translator", "synthesis", "Generate small reference implementations and label generated code.", ["generate_reference_code", "submit_artifact"], []),
    ("experiment_designer", "Experiment Designer", "synthesis", "Suggest bounded tests to validate an algorithm or extracted claim.", ["design_experiment", "submit_artifact"], []),
]

DENIED = ["publish_algorithm", "modify_canonical_archive", "unrestricted_shell"]

INPUT = {
    "type": "object",
    "properties": {
        "artifact_id": {"type": "string"},
        "locator": {"type": "string"},
        "url": {"type": "string"},
        "source_text": {"type": "string"},
        "objective": {"type": "string"},
        "extraction": {"type": "object"},
        "claims": {"type": "array"},
        "language": {"type": "string"},
        "source": {"type": "string"},
        "max_depth": {"type": "integer"},
        "normalized": {"type": "object"},
    },
}

OUTPUTS = {
    "source_scout": {"type": "object", "required": ["sources"], "properties": {"sources": {"type": "array"}, "objective": {"type": "string"}}},
    "web_collector": {"type": "object", "required": ["locator"], "properties": {"locator": {"type": "string"}, "text": {"type": "string"}, "blocked": {"type": "boolean"}, "reason": {"type": "string"}, "resolved": {"type": "string"}, "bytes": {"type": "integer"}}},
    "paper_analyst": {"type": "object", "required": ["title"], "properties": {"title": {"type": "string"}, "authors": {"type": "array"}, "abstract": {"type": "string"}, "sections": {"type": "array"}, "equations": {"type": "array"}, "algorithms": {"type": "array"}, "references": {"type": "array"}, "methodology_clues": {"type": "array"}, "origin": {"type": "string"}}},
    "repository_scout": {"type": "object", "required": ["repository"], "properties": {"repository": {"type": "string"}, "path": {"type": "string"}, "commit_ref": {"type": "string"}, "language": {"type": "string"}, "license": {"type": "string"}, "title": {"type": "string"}}},
    "citation_walker": {"type": "object", "required": ["citations"], "properties": {"citations": {"type": "array"}, "max_depth": {"type": "integer"}, "bounded": {"type": "boolean"}}},
    "algorithm_detector": {"type": "object", "required": ["verdict"], "properties": {"verdict": {"type": "string", "enum": ["YES", "NO", "UNCERTAIN"]}, "reason": {"type": "string"}, "title": {"type": "string"}}},
    "algorithm_extractor": {"type": "object", "properties": {"extraction": {"type": "object"}, "origin": {"type": "string"}, "ambiguous": {"type": "boolean"}, "extracted": {"type": "boolean"}, "reason": {"type": "string"}}},
    "math_analyst": {"type": "object", "required": ["equations"], "properties": {"equations": {"type": "array"}, "variables": {"type": "array"}, "operators": {"type": "array"}, "recurrences": {"type": "array"}, "constraints": {"type": "array"}, "assumptions": {"type": "array"}, "objective_functions": {"type": "array"}, "update_rules": {"type": "array"}, "ambiguous": {"type": "boolean"}}},
    "code_analyst": {"type": "object", "required": ["algorithm_structure"], "properties": {"algorithm_structure": {"type": "string"}, "state": {"type": "array"}, "data_flow": {"type": "array"}, "important_functions": {"type": "array"}, "implementation_constraints": {"type": "array"}, "complexity_clues": {"type": "array"}, "language": {"type": "string"}}},
    "complexity_analyst": {"type": "object", "required": ["origin"], "properties": {"time_complexity": {"type": "string"}, "space_complexity": {"type": "string"}, "scaling": {"type": "string"}, "origin": {"type": "string", "enum": ["SOURCE_STATED", "DERIVED", "UNKNOWN"]}}},
    "assumption_analyst": {"type": "object", "required": ["assumptions"], "properties": {"assumptions": {"type": "array"}}},
    "failure_mode_analyst": {"type": "object", "required": ["failure_conditions"], "properties": {"edge_cases": {"type": "array"}, "numerical_instability": {"type": "array"}, "invalid_input_conditions": {"type": "array"}, "convergence_risks": {"type": "array"}, "known_limitations": {"type": "array"}, "failure_conditions": {"type": "array"}}},
    "provenance_checker": {"type": "object", "required": ["claims"], "properties": {"claims": {"type": "array"}, "ambiguous": {"type": "boolean"}, "unsupported": {"type": "integer"}}},
    "cross_source_verifier": {"type": "object", "required": ["certainty"], "properties": {"independent_sources": {"type": "array"}, "certainty": {"type": "string"}, "note": {"type": "string"}}},
    "math_checker": {"type": "object", "required": ["result"], "properties": {"result": {"type": "string", "enum": ["PASS", "FAIL", "UNCERTAIN", "NOT_TESTED"]}, "reason": {"type": "string"}, "failed": {"type": "array"}, "checked": {"type": "string"}, "equations": {"type": "array"}}},
    "code_verifier": {"type": "object", "required": ["result"], "properties": {"result": {"type": "string", "enum": ["PASS", "FAIL", "UNCERTAIN", "NOT_TESTED"]}, "reason": {"type": "string"}, "stdout": {"type": "string"}, "stderr": {"type": "string"}}},
    "hallucination_challenger": {"type": "object", "required": ["challenges"], "properties": {"challenges": {"type": "array"}, "challenged": {"type": "boolean"}}},
    "deduplicator": {"type": "object", "required": ["verdict"], "properties": {"verdict": {"type": "string", "enum": ["NEW", "DUPLICATE", "POSSIBLE_DUPLICATE", "VARIANT"]}, "matches": {"type": "array"}, "candidate": {"type": "object"}}},
    "relationship_mapper": {"type": "object", "required": ["algorithms"], "properties": {"algorithms": {"type": "array"}, "research": {"type": "array"}, "projects": {"type": "array"}, "notes": {"type": "array"}, "labs": {"type": "array"}}},
    "structural_matcher": {"type": "object", "required": ["motifs"], "properties": {"motifs": {"type": "array"}, "equivalence_claimed": {"type": "boolean"}, "note": {"type": "string"}}},
    "domain_classifier": {"type": "object", "required": ["domain"], "properties": {"domain": {"type": "string"}, "tags": {"type": "array"}}},
    "use_case_mapper": {"type": "object", "required": ["known_uses", "potential_uses"], "properties": {"known_uses": {"type": "array"}, "potential_uses": {"type": "array"}, "separated": {"type": "boolean"}}},
    "normalizer": {"type": "object", "required": ["normalized"], "properties": {"normalized": {"type": "object"}}},
    "summarizer": {"type": "object", "required": ["card"], "properties": {"card": {"type": "string"}, "detail": {"type": "string"}, "title": {"type": "string"}}},
    "code_translator": {"type": "object", "required": ["code", "label"], "properties": {"language": {"type": "string"}, "code": {"type": "string"}, "label": {"type": "string"}}},
    "experiment_designer": {"type": "object", "required": ["proposed_tests"], "properties": {"proposed_tests": {"type": "array"}, "validation_status": {"type": "string"}, "note": {"type": "string"}}},
}

SMOKE = {
    "source_scout": {"objective": "shortest path weighted graphs"},
    "web_collector": {"locator": "fixture://shortest-path/dijkstra.md"},
}


def write(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(data, (dict, list)):
        path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    else:
        path.write_text(str(data), encoding="utf-8")


def main() -> None:
    ids = [item[0] for item in AGENTS]
    write(ROOT / "registry" / "agents.json", {"schema_version": "1.0.0", "agents": ids})

    tools = [
        {"id": "list_agents", "description": "List registered agents", "session_required": False, "args": {"type": "object"}},
        {"id": "describe_agent", "description": "Describe one agent", "session_required": False, "args": {"type": "object", "required": ["agent_id"]}},
        {"id": "list_tools", "description": "List tools", "session_required": False, "args": {"type": "object"}},
        {"id": "describe_tool", "description": "Describe one tool", "session_required": False, "args": {"type": "object", "required": ["tool_id"]}},
        {"id": "start_session", "description": "Create a research session", "session_required": False, "args": {"type": "object", "required": ["objective"]}},
        {"id": "get_session", "description": "Inspect a session", "session_required": True, "args": {"type": "object", "required": ["session_id"]}},
        {"id": "list_sessions", "description": "List sessions", "session_required": False, "args": {"type": "object"}},
        {"id": "run_agent", "description": "Run a registered agent", "session_required": True, "args": {"type": "object", "required": ["session_id", "agent_id"]}},
        {"id": "get_agent_status", "description": "Get run status", "session_required": False, "args": {"type": "object", "required": ["run_id"]}},
        {"id": "get_agent_result", "description": "Get run result", "session_required": False, "args": {"type": "object", "required": ["run_id"]}},
        {"id": "send_agent_message", "description": "Send a typed inter-agent message", "session_required": True, "args": {"type": "object"}},
        {"id": "get_agent_messages", "description": "List session messages", "session_required": True, "args": {"type": "object"}},
        {"id": "submit_review_candidate", "description": "Submit a private extraction candidate", "session_required": True, "args": {"type": "object"}},
        {"id": "pause_session", "description": "Pause a session", "session_required": True, "args": {"type": "object"}},
        {"id": "resume_session", "description": "Resume a session", "session_required": True, "args": {"type": "object"}},
        {"id": "stop_session", "description": "Stop a session", "session_required": True, "args": {"type": "object"}},
        {"id": "get_session_events", "description": "List session events", "session_required": True, "args": {"type": "object"}},
        {"id": "get_session_receipt", "description": "Write and return the session receipt", "session_required": True, "args": {"type": "object"}},
        {"id": "get_overview", "description": "Runtime overview counts", "session_required": False, "args": {"type": "object"}},
        {"id": "list_artifacts", "description": "List artifacts", "session_required": True, "args": {"type": "object"}},
        {"id": "get_artifact", "description": "Read an artifact", "session_required": False, "args": {"type": "object"}},
        {"id": "list_runs", "description": "List runs", "session_required": True, "args": {"type": "object"}},
        {"id": "submit_artifact", "description": "Internal artifact write used by agents", "session_required": True, "args": {"type": "object"}},
        {"id": "request_math_analysis", "description": "Ask math_analyst for analysis", "session_required": True, "args": {"type": "object"}},
    ]
    aliases = [
        "search_sources", "retrieve_source", "inspect_document", "inspect_repository",
        "detect_algorithm", "extract_algorithm", "analyze_math", "analyze_code",
        "analyze_complexity", "analyze_assumptions", "analyze_failures",
        "verify_provenance", "verify_math", "verify_code", "challenge_claims",
        "find_duplicates", "find_relationships", "find_structural_matches",
        "classify_domain", "map_use_cases", "normalize_candidate", "summarize_candidate",
        "generate_reference_code", "design_experiment",
    ]
    for name in aliases:
        tools.append({"id": name, "description": f"Supervisor alias `{name}`", "session_required": True, "args": {"type": "object"}})
    write(ROOT / "registry" / "tools.json", {"schema_version": "1.0.0", "tools": tools})
    write(
        ROOT / "registry" / "permissions.json",
        {
            "publication_allowed": False,
            "live_fetch_enabled": False,
            "allowed_hosts": [],
            "denied_hosts": ["localhost"],
            "max_fetch_bytes": 250000,
            "fetch_timeout_seconds": 8,
            "user_agent": "Jackson-Agent-System/1.0 (local research; no bypass)",
            "code_languages": ["python"],
            "code_timeout_seconds": 3,
            "code_output_limit": 4096,
            "globally_denied_tools": DENIED + ["write_file", "run_shell", "approve_extraction"],
        },
    )

    write(
        ROOT / "schemas" / "agent.schema.json",
        {
            "type": "object",
            "required": ["id", "name", "version", "purpose", "family", "allowed_tools", "denied_tools"],
            "properties": {
                "id": {"type": "string"},
                "name": {"type": "string"},
                "version": {"type": "string"},
                "purpose": {"type": "string"},
                "family": {"type": "string"},
                "inputs": {"type": "array"},
                "outputs": {"type": "array"},
                "allowed_tools": {"type": "array"},
                "denied_tools": {"type": "array"},
                "can_message": {"type": "array"},
                "timeout_seconds": {"type": "integer"},
                "max_retries": {"type": "integer"},
                "enabled": {"type": "boolean"},
            },
        },
    )
    write(
        ROOT / "schemas" / "tool.schema.json",
        {
            "type": "object",
            "required": ["id", "description"],
            "properties": {
                "id": {"type": "string"},
                "description": {"type": "string"},
                "session_required": {"type": "boolean"},
                "args": {"type": "object"},
            },
        },
    )
    for name in ["session", "message", "artifact", "extraction", "receipt"]:
        write(ROOT / "schemas" / f"{name}.schema.json", {"type": "object"})

    for agent_id, name, family, purpose, allowed, can_message in AGENTS:
        directory = ROOT / "agents" / agent_id
        write(
            directory / "agent.json",
            {
                "id": agent_id,
                "name": name,
                "version": "1.0.0",
                "purpose": purpose,
                "family": family,
                "inputs": ["source_document"],
                "outputs": ["artifact"],
                "allowed_tools": allowed,
                "denied_tools": DENIED,
                "can_message": can_message,
                "timeout_seconds": 120,
                "max_retries": 2,
                "enabled": True,
            },
        )
        write(directory / "input.schema.json", INPUT)
        write(directory / "output.schema.json", OUTPUTS[agent_id])
        write(directory / "smoke_input.json", SMOKE.get(agent_id, {"artifact_id": "$SOURCE"}))
        write(directory / "smoke_output.json", {"note": "validated against output.schema.json at runtime"})

    print(f"generated {len(AGENTS)} agent contracts")


if __name__ == "__main__":
    main()
