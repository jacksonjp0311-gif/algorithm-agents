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
        #[arg(long, default_value = "Extract reconstructable algorithms from this source")]
        objective: String,
    },
    #[command(subcommand)]
    Archive(ArchiveCmd),
    #[command(subcommand)]
    Agent(AgentCmd),
    #[command(subcommand)]
    Session(SessionCmd),
    Run {
        agent: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        input: Option<String>,
    },
    Smoke {
        #[arg(long)]
        agent: Option<String>,
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
    Accept { id: String },
    /// Reject a queued candidate
    Reject {
        id: String,
        #[arg(long, default_value = "rejected by operator")]
        reason: String,
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
    Inspect { id: String },
    Pause { id: String },
    Stop { id: String },
    List,
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
    let archive = Arc::new(FileArchive::open(&data_dir).await?);
    let mut runtime = AgentRuntime::open(root, data_dir, pool, archive.clone()).await?;
    runtime.permissions.apply_runtime_overrides(cli.live);

    match cli.command {
        Command::Find { query } => print_json(pipeline::find_sources(&runtime, &query).await?),
        Command::Harvest { objective, limit } => {
            print_json(pipeline::harvest(&runtime, &objective, limit).await?)
        }
        Command::Scrape { locator, objective } => {
            print_json(pipeline::scrape(&runtime, &locator, &objective).await?)
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
        Command::Archive(ArchiveCmd::Accept { id }) => {
            print_json(serde_json::to_value(archive.accept(&id).await?)?)
        }
        Command::Archive(ArchiveCmd::Reject { id, reason }) => {
            print_json(serde_json::to_value(archive.reject(&id, &reason).await?)?)
        }
        Command::Agent(AgentCmd::List) => print_json(dispatch(&runtime, "list_agents", json!({})).await?),
        Command::Agent(AgentCmd::Describe { id }) => {
            print_json(dispatch(&runtime, "describe_agent", json!({ "agent_id": id })).await?)
        }
        Command::Session(SessionCmd::Create { objective, supervisor }) => {
            print_json(
                dispatch(
                    &runtime,
                    "start_session",
                    json!({ "objective": objective, "supervisor": supervisor }),
                )
                .await?,
            )
        }
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
        Command::Smoke { agent } => {
            let report = smoke::smoke_all(&runtime, agent.as_deref()).await?;
            println!("{}", smoke::format_smoke_report(&report));
            if report.get("verdict").and_then(|v| v.as_str()) != Some("PASS") {
                std::process::exit(1);
            }
        }
        Command::Ui { bind, no_open } => {
            let addr: std::net::SocketAddr = bind.parse()?;
            let url = format!("http://{addr}/");
            println!("Algorithm archive UI → {url}");
            println!("Cards read from {}", runtime.data_dir.join("archive").display());
            if !no_open {
                agent_system::ui::open_browser(&url);
            }
            agent_system::ui::serve(runtime.root.clone(), archive, addr).await?;
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
    println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()));
}
