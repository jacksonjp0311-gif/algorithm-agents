use std::path::PathBuf;
use std::sync::Arc;

use agent_system::archive::FileArchive;
use agent_system::persist::{self, connect_sqlite};
use agent_system::runtime::AgentRuntime;
use agent_system::{dispatch, pipeline, smoke};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "algo",
    about = "Discover, extract, review, and save algorithms with a bounded multi-agent runtime",
    version
)]
struct Cli {
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    database: Option<String>,
    /// Allow http(s) retrieval. Private hosts stay denied.
    #[arg(long)]
    live: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Search bundled fixtures, dropped sources, and the local archive
    Find {
        query: String,
    },
    /// Scout matching sources, extract algorithms, and save review candidates
    Harvest {
        objective: String,
        #[arg(long, default_value_t = 4)]
        limit: usize,
    },
    /// Fetch one locator or URL, extract, and save a review candidate
    Scrape {
        locator: String,
        #[arg(
            long,
            default_value = "Extract reconstructable algorithms from this source"
        )]
        objective: String,
    },
    /// Search arXiv / the live web and extract algorithm candidates
    Hunt {
        /// Topic to hunt, e.g. "physics" or "monte carlo"
        query: String,
        #[arg(long, default_value_t = true)]
        arxiv: bool,
        #[arg(long, default_value_t = true)]
        web: bool,
        #[arg(long, default_value_t = 6)]
        limit: usize,
        #[arg(long)]
        no_arxiv: bool,
        #[arg(long)]
        no_web: bool,
    },
    #[command(subcommand)]
    Archive(ArchiveCmd),
    #[command(subcommand)]
    Agent(AgentCmd),
    #[command(subcommand)]
    Session(SessionCmd),
    #[command(subcommand)]
    Graph(GraphCmd),
    #[command(subcommand)]
    Emergent(EmergentCmd),
    #[command(subcommand)]
    Model(ModelCmd),
    #[command(subcommand)]
    Eval(EvalCmd),
    #[command(subcommand)]
    Bundle(BundleCmd),
    /// Check runtime, archive, graph, and registry integrity
    Doctor,
    /// Print machine-readable API, model, role, and MCP capabilities
    Capabilities,
    /// Run the Model Context Protocol server over stdio
    Mcp,
    Run {
        agent: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        input: Option<String>,
    },
    /// Turn a human sentence into a scrape / hunt / harvest
    Do {
        intent: String,
        #[arg(long, default_value_t = 3)]
        limit: usize,
    },
    /// Print the command card an AI operator should run
    Cmds,
    /// Print the model-supervisor contract or execute one audited envelope
    Supervisor {
        #[arg(long)]
        request: Option<String>,
    },
    Smoke {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        process: bool,
    },
    Demo {
        name: String,
    },
    /// Open the local archive card UI
    Ui {
        #[arg(long, default_value = "127.0.0.1:8791")]
        bind: String,
        #[arg(long)]
        no_open: bool,
        /// Permit a non-loopback bind. Use only behind an authenticated proxy.
        #[arg(long)]
        allow_remote: bool,
        /// Supply an operator token (primarily for controlled automation).
        #[arg(long, hide = true)]
        operator_token: Option<String>,
    },
    Tool {
        name: String,
        #[arg(long)]
        args: Option<String>,
    },
}

#[derive(Subcommand)]
enum ArchiveCmd {
    /// List queued candidates and accepted algorithms
    List,
    /// Show one queued candidate
    Show { id: String },
    /// Promote a PENDING candidate into the local archive
    Accept {
        id: String,
        #[arg(long, default_value = "reviewed and accepted by operator")]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
        #[arg(long)]
        override_contested: bool,
    },
    /// Reject a queued candidate
    Reject {
        id: String,
        #[arg(long, default_value = "rejected by operator")]
        reason: String,
    },
    /// List immutable canonical archive revisions
    History,
    /// Restore an immutable canonical revision
    Rollback {
        revision: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
    },
}

#[derive(Subcommand)]
enum AgentCmd {
    List,
    Describe { id: String },
}

#[derive(Subcommand)]
enum SessionCmd {
    Create {
        #[arg(long)]
        objective: String,
        #[arg(long, default_value = "manual")]
        supervisor: String,
    },
    Inspect {
        id: String,
    },
    Pause {
        id: String,
    },
    Stop {
        id: String,
    },
    List,
}

#[derive(Subcommand)]
enum GraphCmd {
    Snapshot,
    Ontology,
    Integrity,
    Proposals,
    Disputes,
    OpenDispute {
        #[arg(long)]
        input: PathBuf,
    },
    ResolveDispute {
        id: String,
        #[arg(long)]
        reviewer: Option<String>,
        #[arg(long)]
        resolution: String,
    },
    Accept {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
    },
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
    },
}

#[derive(Subcommand)]
enum EmergentCmd {
    List,
    Propose {
        #[arg(long)]
        input: PathBuf,
    },
    Challenge {
        id: String,
        #[arg(long)]
        input: PathBuf,
    },
    Review {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
        #[arg(long)]
        reject: bool,
    },
    Logs,
    LogCreate {
        #[arg(long)]
        input: PathBuf,
    },
    LogReview {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        reviewer: Option<String>,
        #[arg(long)]
        reject: bool,
    },
    Scan {
        #[arg(long, default_value = "operator-requested-scanner")]
        proposed_by: String,
    },
}

#[derive(Subcommand)]
enum ModelCmd {
    Capabilities,
    Invoke {
        #[arg(long)]
        input: PathBuf,
    },
    History,
}

#[derive(Subcommand)]
enum EvalCmd {
    Run,
    History,
}

#[derive(Subcommand)]
enum BundleCmd {
    Export {
        #[arg(long)]
        output: PathBuf,
    },
    Inspect {
        #[arg(long)]
        input: PathBuf,
    },
    Import {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "operator import")]
        source: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = AgentRuntime::discover_root(cli.root.as_deref());
    let cwd = std::env::current_dir()?;
    let data_dir = cli
        .data_dir
        .unwrap_or_else(|| persist::default_data_dir(&cwd));
    let database = cli.database.unwrap_or_else(|| {
        format!("sqlite://{}", data_dir.join("agent.db").to_string_lossy()).replace('\\', "/")
    });
    let pool = connect_sqlite(&database).await?;
    let archive = Arc::new(
        FileArchive::open(&data_dir)
            .await?
            .with_audit_pool(pool.clone()),
    );
    let mut runtime = AgentRuntime::open(root, data_dir, pool, archive.clone()).await?;
    runtime.permissions.apply_runtime_overrides(cli.live);

    match cli.command {
        Command::Find { query } => print_json(pipeline::find_sources(&runtime, &query).await?),
        Command::Harvest { objective, limit } => {
            print_json(pipeline::harvest(&runtime, &objective, limit).await?)
        }
        Command::Scrape { locator, objective } => {
            runtime.permissions.fetch_timeout_seconds =
                runtime.permissions.fetch_timeout_seconds.max(20);
            runtime.permissions.max_fetch_bytes =
                runtime.permissions.max_fetch_bytes.max(1_200_000);
            print_json(pipeline::scrape(&runtime, &locator, &objective).await?)
        }
        Command::Hunt {
            query,
            arxiv,
            web,
            limit,
            no_arxiv,
            no_web,
        } => {
            runtime.permissions.fetch_timeout_seconds =
                runtime.permissions.fetch_timeout_seconds.max(20);
            runtime.permissions.max_fetch_bytes =
                runtime.permissions.max_fetch_bytes.max(1_200_000);
            print_json(
                agent_system::hunt::hunt(
                    &runtime,
                    agent_system::hunt::HuntSpec {
                        query,
                        arxiv: arxiv && !no_arxiv,
                        web: web && !no_web,
                        limit,
                    },
                )
                .await?,
            )
        }
        Command::Archive(ArchiveCmd::List) => {
            let queue = archive.list_queue().await;
            let accepted = archive.list_accepted().await;
            print_json(json!({
                "queue": queue,
                "accepted": accepted
            }))
        }
        Command::Archive(ArchiveCmd::Show { id }) => {
            print_json(serde_json::to_value(archive.get_candidate(&id).await?)?)
        }
        Command::Archive(ArchiveCmd::Accept {
            id,
            reason,
            reviewer,
            override_contested,
        }) => {
            let expected_candidate_hash = archive.candidate_hash(&id).await?;
            print_json(serde_json::to_value(
                archive
                    .accept_with_review(
                        &id,
                        agent_system::archive::ReviewDecision {
                            reviewer: reviewer.unwrap_or_else(operator_name),
                            reason,
                            expected_candidate_hash,
                            override_contested,
                        },
                    )
                    .await?,
            )?)
        }
        Command::Archive(ArchiveCmd::Reject { id, reason }) => {
            print_json(serde_json::to_value(archive.reject(&id, &reason).await?)?)
        }
        Command::Archive(ArchiveCmd::History) => {
            print_json(json!({ "revisions": archive.revision_history().await? }))
        }
        Command::Archive(ArchiveCmd::Rollback {
            revision,
            reason,
            reviewer,
        }) => print_json(
            archive
                .rollback(&revision, &reviewer.unwrap_or_else(operator_name), &reason)
                .await?,
        ),
        Command::Agent(AgentCmd::List) => {
            print_json(dispatch(&runtime, "list_agents", json!({})).await?)
        }
        Command::Agent(AgentCmd::Describe { id }) => {
            print_json(dispatch(&runtime, "describe_agent", json!({ "agent_id": id })).await?)
        }
        Command::Session(SessionCmd::Create {
            objective,
            supervisor,
        }) => print_json(
            dispatch(
                &runtime,
                "start_session",
                json!({ "objective": objective, "supervisor": supervisor }),
            )
            .await?,
        ),
        Command::Session(SessionCmd::Inspect { id }) => {
            print_json(dispatch(&runtime, "get_session", json!({ "session_id": id })).await?)
        }
        Command::Session(SessionCmd::Pause { id }) => {
            print_json(dispatch(&runtime, "pause_session", json!({ "session_id": id })).await?)
        }
        Command::Session(SessionCmd::Stop { id }) => {
            print_json(dispatch(&runtime, "stop_session", json!({ "session_id": id })).await?)
        }
        Command::Session(SessionCmd::List) => {
            print_json(dispatch(&runtime, "list_sessions", json!({})).await?)
        }
        Command::Graph(GraphCmd::Snapshot) => {
            print_json(agent_system::graph::graph_snapshot(&runtime.pool, false).await?)
        }
        Command::Graph(GraphCmd::Ontology) => print_json(agent_system::graph::ontology()),
        Command::Graph(GraphCmd::Integrity) => {
            print_json(agent_system::graph::integrity_report(&runtime.pool).await?)
        }
        Command::Graph(GraphCmd::Proposals) => print_json(json!({
            "proposals": agent_system::graph::list_proposals(&runtime.pool).await?
        })),
        Command::Graph(GraphCmd::Disputes) => print_json(json!({
            "disputes": agent_system::graph::list_disputes(&runtime.pool).await?
        })),
        Command::Graph(GraphCmd::OpenDispute { input }) => {
            let value = read_json_file(&input).await?;
            print_json(serde_json::to_value(
                agent_system::graph::open_dispute(
                    &runtime.pool,
                    required_json_string(&value, "target_type")?,
                    required_json_string(&value, "target_id")?,
                    required_json_string(&value, "claim")?,
                    value.get("evidence").cloned().unwrap_or_else(|| json!([])),
                    required_json_string(&value, "opened_by")?,
                )
                .await?,
            )?)
        }
        Command::Graph(GraphCmd::ResolveDispute {
            id,
            reviewer,
            resolution,
        }) => print_json(serde_json::to_value(
            agent_system::graph::resolve_dispute(
                &runtime.pool,
                &id,
                &reviewer.unwrap_or_else(operator_name),
                &resolution,
            )
            .await?,
        )?),
        Command::Graph(GraphCmd::Accept {
            id,
            reason,
            reviewer,
        }) => print_json(serde_json::to_value(
            agent_system::graph::review_relationship(
                &runtime.pool,
                &id,
                true,
                &reviewer.unwrap_or_else(operator_name),
                &reason,
            )
            .await?,
        )?),
        Command::Graph(GraphCmd::Reject {
            id,
            reason,
            reviewer,
        }) => print_json(serde_json::to_value(
            agent_system::graph::review_relationship(
                &runtime.pool,
                &id,
                false,
                &reviewer.unwrap_or_else(operator_name),
                &reason,
            )
            .await?,
        )?),
        Command::Emergent(EmergentCmd::List) => print_json(json!({
            "hypotheses": agent_system::emergent::list_hypotheses(&runtime.pool, true).await?
        })),
        Command::Emergent(EmergentCmd::Propose { input }) => {
            let request = serde_json::from_value(read_json_file(&input).await?)?;
            print_json(agent_system::emergent::propose_hypothesis(&runtime.pool, request).await?)
        }
        Command::Emergent(EmergentCmd::Challenge { id, input }) => {
            let value = read_json_file(&input).await?;
            print_json(
                agent_system::emergent::challenge_hypothesis(
                    &runtime.pool,
                    &id,
                    required_json_string(&value, "challenger")?,
                    required_json_string(&value, "verdict")?,
                    required_json_string(&value, "rationale")?,
                    value.get("evidence").cloned().unwrap_or_else(|| json!([])),
                )
                .await?,
            )
        }
        Command::Emergent(EmergentCmd::Review {
            id,
            reason,
            reviewer,
            reject,
        }) => print_json(
            agent_system::emergent::review_hypothesis(
                &runtime.pool,
                &id,
                !reject,
                &reviewer.unwrap_or_else(operator_name),
                &reason,
            )
            .await?,
        ),
        Command::Emergent(EmergentCmd::Logs) => print_json(json!({
            "logs": agent_system::emergent::list_research_logs(&runtime.pool, true).await?
        })),
        Command::Emergent(EmergentCmd::LogCreate { input }) => {
            let request = serde_json::from_value(read_json_file(&input).await?)?;
            print_json(agent_system::emergent::create_research_log(&runtime.pool, request).await?)
        }
        Command::Emergent(EmergentCmd::LogReview {
            id,
            reason,
            reviewer,
            reject,
        }) => print_json(
            agent_system::emergent::review_research_log(
                &runtime.pool,
                &id,
                !reject,
                &reviewer.unwrap_or_else(operator_name),
                &reason,
            )
            .await?,
        ),
        Command::Emergent(EmergentCmd::Scan { proposed_by }) => print_json(json!({
            "hypotheses": agent_system::emergent::scan_for_missing_links(&runtime.pool, &proposed_by).await?,
            "canonical_publications": 0
        })),
        Command::Model(ModelCmd::Capabilities) => print_json(agent_system::model::capabilities()),
        Command::Model(ModelCmd::Invoke { input }) => {
            let request = serde_json::from_value(read_json_file(&input).await?)?;
            print_json(serde_json::to_value(
                agent_system::model::invoke(&runtime.pool, request).await?,
            )?)
        }
        Command::Model(ModelCmd::History) => print_json(json!({
            "invocations": agent_system::model::list_invocations(&runtime.pool).await?
        })),
        Command::Eval(EvalCmd::Run) => {
            let report = agent_system::evaluation::run_corpus(&runtime.pool, &runtime.root).await?;
            let passed = report.get("passed") == Some(&Value::Bool(true));
            print_json(report);
            if !passed {
                std::process::exit(1);
            }
        }
        Command::Eval(EvalCmd::History) => print_json(json!({
            "evaluations": agent_system::evaluation::history(&runtime.pool).await?
        })),
        Command::Bundle(BundleCmd::Export { output }) => {
            print_json(agent_system::bundle::export(&archive, &runtime.pool, &output).await?)
        }
        Command::Bundle(BundleCmd::Inspect { input }) => {
            print_json(agent_system::bundle::inspect(&input).await?)
        }
        Command::Bundle(BundleCmd::Import { input, source }) => {
            print_json(agent_system::bundle::import_private(&runtime.pool, &input, &source).await?)
        }
        Command::Doctor => print_json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "runtime": runtime.overview().await?,
            "archive": archive.integrity_report().await?,
            "graph": agent_system::graph::integrity_report(&runtime.pool).await?,
            "registry_valid": runtime.registry.invalid.is_empty(),
            "publication_boundary": "HUMAN_OPERATOR_ONLY"
        })),
        Command::Capabilities => print_json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "supervisor": agent_system::attach_contract(),
            "models": agent_system::model::capabilities(),
            "mcp": agent_system::mcp::descriptor(),
            "graph": agent_system::graph::ontology()
        })),
        Command::Mcp => agent_system::mcp::serve(Arc::new(runtime)).await?,
        Command::Run {
            agent,
            session,
            artifact,
            input,
        } => {
            let mut payload = if let Some(raw) = input {
                serde_json::from_str(&raw)?
            } else {
                json!({})
            };
            if let Some(artifact) = artifact {
                payload["artifact_id"] = json!(artifact);
            }
            print_json(
                dispatch(
                    &runtime,
                    "run_agent",
                    json!({ "session_id": session, "agent_id": agent, "input": payload }),
                )
                .await?,
            )
        }
        Command::Do { intent, limit } => {
            print_json(agent_system::intent::execute_intent(&mut runtime, &intent, limit).await?)
        }
        Command::Cmds => print_json(agent_system::intent::command_card()),
        Command::Supervisor { request } => {
            if let Some(raw) = request {
                let envelope: agent_system::supervisor::SupervisorEnvelope =
                    serde_json::from_str(&raw)?;
                print_json(agent_system::supervisor::execute_envelope(&runtime, envelope).await?)
            } else {
                print_json(agent_system::attach_contract())
            }
        }
        Command::Smoke { agent, process } => {
            if process {
                let report = smoke::smoke_process(&runtime).await?;
                println!(
                    "{}",
                    smoke::format_smoke_report(report.get("agents").unwrap_or(&report))
                );
                println!(
                    "PROCESS harvest={} demo={} verdict={}",
                    report
                        .get("harvest_ok")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    report
                        .get("demo_ok")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    report
                        .get("verdict")
                        .and_then(|v| v.as_str())
                        .unwrap_or("FAIL")
                );
                if report.get("verdict").and_then(|v| v.as_str()) != Some("PASS") {
                    std::process::exit(1);
                }
            } else {
                let report = smoke::smoke_all(&runtime, agent.as_deref()).await?;
                println!("{}", smoke::format_smoke_report(&report));
                if report.get("verdict").and_then(|v| v.as_str()) != Some("PASS") {
                    std::process::exit(1);
                }
            }
        }
        Command::Ui {
            bind,
            no_open,
            allow_remote,
            operator_token,
        } => {
            let addr: std::net::SocketAddr = bind.parse()?;
            let operator_token =
                operator_token.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
            let url = format!("http://{addr}/?operator_token={operator_token}");
            println!("Algorithm archive UI → {url}");
            println!(
                "Cards read from {}",
                runtime.data_dir.join("archive").display()
            );
            if !no_open {
                agent_system::ui::open_browser(&url);
            }
            agent_system::ui::serve(
                runtime.root.clone(),
                archive,
                runtime.pool.clone(),
                addr,
                operator_token,
                allow_remote,
            )
            .await?;
        }
        Command::Demo { name } => {
            let value = match name.as_str() {
                "shortest-path" | "1" => agent_system::demo::demo_shortest_path(&runtime).await?,
                "messages" | "2" => agent_system::demo::demo_inter_agent(&runtime).await?,
                "escalation" | "3" => agent_system::demo::demo_escalation(&runtime).await?,
                "harvest" => agent_system::demo::demo_harvest(&runtime).await?,
                "restart" => agent_system::demo::demo_restart(&runtime).await?,
                _ => anyhow::bail!("unknown demo `{name}`"),
            };
            print_json(value);
        }
        Command::Tool { name, args } => {
            let payload = args
                .map(|raw| serde_json::from_str::<Value>(&raw))
                .transpose()?
                .unwrap_or_else(|| json!({}));
            print_json(dispatch(&runtime, &name, payload).await?)
        }
    }
    Ok(())
}

fn print_json(value: Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
    );
}

fn operator_name() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "operator".into())
}

async fn read_json_file(path: &std::path::Path) -> anyhow::Result<Value> {
    let raw = tokio::fs::read_to_string(path).await?;
    Ok(serde_json::from_str(&raw)?)
}

fn required_json_string<'a>(value: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("`{key}` is required"))
}
