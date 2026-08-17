use std::sync::Arc;

use agent_system::demo;
use agent_system::host::MemoryHost;
use agent_system::host::{ArchiveHost, ReviewCandidate};
use agent_system::persist::connect_sqlite;
use agent_system::runtime::AgentRuntime;
use agent_system::smoke;
use agent_system::{dispatch, open_runtime};
use serde_json::json;

fn review_ready(title: &str, short_name: &str, core_idea: &str) -> serde_json::Value {
    json!({
        "title": title,
        "short_name": short_name,
        "core_idea": core_idea,
        "domain": "Graph Algorithms",
        "source": { "type": "test", "title": title, "url": "fixture://test.md" },
        "provenance": { "class": "TEST_FIXTURE", "evidence_notes": "integration test" },
        "publication": { "state": "draft" },
        "validation": {
            "status": "VERIFIED",
            "lifecycle": "REVIEW_READY",
            "mandatory_pass": true,
            "gates": { "fixture": "PASS" }
        }
    })
}

#[test]
fn local_source_traversal_is_denied() {
    let root = AgentRuntime::discover_root(None);
    let result = agent_system::collect::resolve_local(&root, "fixture://../../Cargo.toml");
    assert!(result.is_err());
}

#[test]
fn private_network_targets_are_denied() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    assert!(agent_system::fetch::unsafe_address(IpAddr::V4(
        Ipv4Addr::LOCALHOST
    )));
    assert!(agent_system::fetch::unsafe_address(IpAddr::V4(
        Ipv4Addr::new(169, 254, 169, 254)
    )));
    assert!(agent_system::fetch::unsafe_address(IpAddr::V6(
        Ipv6Addr::LOCALHOST
    )));
    assert!(!agent_system::fetch::unsafe_address(IpAddr::V4(
        Ipv4Addr::new(1, 1, 1, 1)
    )));
}

async fn runtime() -> AgentRuntime {
    let root = AgentRuntime::discover_root(None);
    let dir = tempfile::tempdir().unwrap();
    let db = format!("sqlite://{}/test.db", dir.path().display()).replace('\\', "/");
    let pool = connect_sqlite(&db).await.unwrap();
    let host = MemoryHost {
        published: vec![agent_system::PublishedAlgorithm {
            id: "a-star-search".into(),
            archive_id: "ALG / 0004".into(),
            title: "A* Search".into(),
            short_name: "A*".into(),
            slug: "a-star-search".into(),
            domain: "Graph Algorithms".into(),
            summary: "informed search".into(),
            core_idea: "f(n)=g(n)+h(n)".into(),
            tags: vec!["pathfinding".into()],
        }],
        ..MemoryHost::default()
    };
    let rt = open_runtime(root, dir.path().join("data"), pool, Arc::new(host))
        .await
        .unwrap();
    std::mem::forget(dir);
    rt
}

#[test]
fn arxiv_abs_becomes_api_query() {
    let url = agent_system::collect::canonical_fetch_url("https://arxiv.org/abs/physics/0306182");
    assert!(url.contains("export.arxiv.org"));
    assert!(url.contains("id_list=physics/0306182"));
}

#[test]
fn intent_url_is_direct_scrape() {
    let intent =
        agent_system::intent::parse_intent("scrape this https://arxiv.org/abs/1234.5678 for me");
    assert_eq!(intent.kind, agent_system::intent::IntentKind::Scrape);
    assert_eq!(
        intent.locator.as_deref(),
        Some("https://arxiv.org/abs/1234.5678")
    );
    assert!(intent.live);
}

#[test]
fn intent_arxiv_topic_is_hunt() {
    let intent = agent_system::intent::parse_intent("find monte carlo algorithms on arxiv");
    assert_eq!(intent.kind, agent_system::intent::IntentKind::Hunt);
    assert!(intent.live);
}

#[test]
fn chrome_lines_are_stripped() {
    let text = agent_system::parse::clean_unstructured(
        "Skip to main content\nSearch Submit Donate\nMetropolis methods for quantum Monte Carlo\nThe method samples configurations.",
    );
    assert!(text.to_ascii_lowercase().contains("metropolis"));
    assert!(!text.to_ascii_lowercase().contains("skip to main"));
}

#[test]
fn unstructured_html_is_inferred() {
    let doc = agent_system::parse::parse_source_document(
        "<html><head><title>Heap sort</title></head><body><h1>Heap sort</h1><p>In-place comparison sort.</p><pre>def heap_sort(a):\n    pass</pre></body></html>",
    );
    assert!(doc.title.to_ascii_lowercase().contains("heap"));
    assert_eq!(doc.algorithm_present, Some(true));
}

#[tokio::test]
async fn registry_loads_twenty_six_valid_agents() {
    let rt = runtime().await;
    assert_eq!(rt.registry.invalid.len(), 0, "{:?}", rt.registry.invalid);
    assert_eq!(rt.registry.agents.len(), 26);
}

#[tokio::test]
async fn denied_publish_tool() {
    let rt = runtime().await;
    let err = dispatch(&rt, "publish_algorithm", json!({}))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "denied");
}

#[tokio::test]
async fn tool_session_contract_is_enforced() {
    let rt = runtime().await;
    let result = dispatch(&rt, "get_session", json!({})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn model_supervisor_cannot_publish() {
    let rt = runtime().await;
    let result = agent_system::supervisor::execute_envelope(
        &rt,
        agent_system::supervisor::SupervisorEnvelope {
            protocol_version: "1.0".into(),
            invocation_id: String::new(),
            session_id: String::new(),
            provider: "test-provider".into(),
            model: "test-model".into(),
            tool: "publish_algorithm".into(),
            args: json!({}),
        },
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn canonical_acceptance_creates_revision_receipt_and_graph() {
    let dir = tempfile::tempdir().unwrap();
    let db = format!("sqlite://{}/archive.db", dir.path().display()).replace('\\', "/");
    let pool = connect_sqlite(&db).await.unwrap();
    let archive = agent_system::FileArchive::open(dir.path())
        .await
        .unwrap()
        .with_audit_pool(pool.clone());
    let submitted = archive
        .submit_extraction_candidate(ReviewCandidate {
            session_id: String::new(),
            raw_extraction: "test extraction".into(),
            normalized: review_ready("Test path search", "TPS", "walk edges"),
            review_state: "PENDING".into(),
            verification: json!({ "status": "VERIFIED" }),
        })
        .await
        .unwrap();
    let hash = archive
        .candidate_hash(&submitted.candidate_id)
        .await
        .unwrap();
    let accepted = archive
        .accept_with_review(
            &submitted.candidate_id,
            agent_system::archive::ReviewDecision {
                reviewer: "integration-test".into(),
                reason: "evidence and fields reviewed".into(),
                expected_candidate_hash: hash,
                override_contested: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(accepted.title, "Test path search");
    assert_eq!(archive.revision_history().await.unwrap().len(), 1);
    assert_eq!(
        agent_system::persist::list_publication_receipts(&pool)
            .await
            .unwrap()
            .len(),
        1
    );
    let graph = agent_system::graph::graph_snapshot(&pool, false)
        .await
        .unwrap();
    assert!(
        graph
            .get("nodes")
            .and_then(|value| value.as_array())
            .is_some_and(|nodes| !nodes.is_empty())
    );
}

#[tokio::test]
async fn session_budget_and_states() {
    let rt = runtime().await;
    let session = rt
        .start_session(json!({
            "objective": "budget probe",
            "max_agent_runs": 1,
            "supervisor": "scripted"
        }))
        .await
        .unwrap();
    rt.run_agent(
        &session.session_id,
        "source_scout",
        json!({ "objective": "shortest" }),
    )
    .await
    .unwrap();
    let second = rt
        .run_agent(
            &session.session_id,
            "source_scout",
            json!({ "objective": "shortest" }),
        )
        .await;
    assert!(second.is_err());
}

#[tokio::test]
async fn schema_reject_and_message_type() {
    let rt = runtime().await;
    let session = rt
        .start_session(json!({ "objective": "schema probe", "supervisor": "scripted" }))
        .await
        .unwrap();
    let bad = rt
        .send_message(
            &session.session_id,
            "algorithm_extractor",
            "math_analyst",
            "CHAT",
            "",
            json!({}),
        )
        .await;
    assert!(bad.is_err());
}

#[tokio::test]
async fn submit_blocks_archive_duplicate() {
    let rt = runtime().await;
    let session = rt
        .start_session(json!({ "objective": "dedup gate", "supervisor": "scripted" }))
        .await
        .unwrap();
    let blocked = dispatch(
        &rt,
        "submit_review_candidate",
        json!({
            "session_id": session.session_id,
            "normalized": review_ready("A* Search", "A*", "f=g+h"),
            "raw_extraction": "gate"
        }),
    )
    .await
    .unwrap();
    assert_eq!(blocked.get("blocked").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        blocked.get("verdict").and_then(|v| v.as_str()),
        Some("DUPLICATE")
    );
    let fresh = dispatch(
        &rt,
        "submit_review_candidate",
        json!({
            "session_id": session.session_id,
            "normalized": review_ready("Newton-Raphson iteration", "Newton", "x-f/f'"),
            "raw_extraction": "new"
        }),
    )
    .await
    .unwrap();
    assert_ne!(fresh.get("blocked").and_then(|v| v.as_bool()), Some(true));
    assert!(fresh.get("candidate_id").and_then(|v| v.as_str()).is_some());
}

#[tokio::test]
async fn smoke_compiler_passes() {
    let rt = runtime().await;
    let report = smoke::smoke_all(&rt, None).await.unwrap();
    assert_eq!(
        report.get("verdict").and_then(|v| v.as_str()),
        Some("PASS"),
        "{report}"
    );
}

#[tokio::test]
async fn three_demos_and_restart() {
    let rt = runtime().await;
    let one = demo::demo_shortest_path(&rt).await.unwrap();
    assert_eq!(
        one.get("result").and_then(|v| v.as_str()),
        Some("PASS"),
        "{one}"
    );
    assert_eq!(
        one.get("publication_count").and_then(|v| v.as_i64()),
        Some(0)
    );
    let two = demo::demo_inter_agent(&rt).await.unwrap();
    assert_eq!(
        two.get("inter_agent_communication")
            .and_then(|v| v.as_str()),
        Some("PASS"),
        "{two}"
    );
    let three = demo::demo_escalation(&rt).await.unwrap();
    assert_eq!(
        three.get("human_escalation").and_then(|v| v.as_str()),
        Some("PASS"),
        "{three}"
    );
    let restart = demo::demo_restart(&rt).await.unwrap();
    assert_eq!(
        restart.get("restart").and_then(|v| v.as_str()),
        Some("PASS"),
        "{restart}"
    );
}
