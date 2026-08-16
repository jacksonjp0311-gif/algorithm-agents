use std::sync::Arc;

use agent_system::demo;
use agent_system::host::MemoryHost;
use agent_system::persist::connect_sqlite;
use agent_system::runtime::AgentRuntime;
use agent_system::smoke;
use agent_system::{dispatch, open_runtime};
use serde_json::json;

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
    let intent = agent_system::intent::parse_intent("scrape this https://arxiv.org/abs/1234.5678 for me");
    assert_eq!(intent.kind, agent_system::intent::IntentKind::Scrape);
    assert_eq!(intent.locator.as_deref(), Some("https://arxiv.org/abs/1234.5678"));
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
    let err = dispatch(&rt, "publish_algorithm", json!({})).await.unwrap_err();
    assert_eq!(err.kind(), "denied");
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
            "normalized": { "title": "A* Search", "short_name": "A*", "core_idea": "f=g+h" },
            "raw_extraction": "gate"
        }),
    )
    .await
    .unwrap();
    assert_eq!(blocked.get("blocked").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(blocked.get("verdict").and_then(|v| v.as_str()), Some("DUPLICATE"));
    let fresh = dispatch(
        &rt,
        "submit_review_candidate",
        json!({
            "session_id": session.session_id,
            "normalized": { "title": "Newton-Raphson iteration", "short_name": "Newton", "core_idea": "x-f/f'" },
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
    assert_eq!(one.get("result").and_then(|v| v.as_str()), Some("PASS"), "{one}");
    assert_eq!(one.get("publication_count").and_then(|v| v.as_i64()), Some(0));
    let two = demo::demo_inter_agent(&rt).await.unwrap();
    assert_eq!(
        two.get("inter_agent_communication").and_then(|v| v.as_str()),
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
    assert_eq!(restart.get("restart").and_then(|v| v.as_str()), Some("PASS"), "{restart}");
}
