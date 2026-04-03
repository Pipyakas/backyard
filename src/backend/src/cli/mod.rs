use clap::{Parser, Subcommand};
use std::process::Command;
use anyhow::{Result, Context};

#[derive(Parser)]
#[command(name = "backyard")]
#[command(about = "Backyard backend and CLI tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Query and list available models
    Models,
    /// Manage the backyard systemd service
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Download a model via the API
    Download {
        repo_id: String,
        filename: String,
    },
}

#[derive(Subcommand)]
pub enum ServiceAction {
    /// Install the systemd user service
    Install,
    /// Start the backyard service
    Start,
    /// Stop the backyard service
    Stop,
    /// Show service status
    Status,
    /// Show service logs
    Log,
}

pub async fn handle_command(cli: Cli, state: &crate::state::State) -> Result<()> {
    match cli.command {
        Some(Commands::Models) => {
            let models = state.list_models()?;
            if models.is_empty() {
                println!("No models found.");
            } else {
                println!("{:<5} {:<30} {:<20} {:<10} {:<10}", "ID", "Name", "Family", "Quant", "Lib ID");
                for m in models {
                    let family = m.family.unwrap_or_else(|| "-".to_string());
                    let quant = m.quant.unwrap_or_else(|| "-".to_string());
                    println!("{:<5} {:<30} {:<20} {:<10} {:<10}", m.id, m.name, family, quant, m.library_id);
                }
            }
            Ok(())
        }
        Some(Commands::Service { action }) => handle_service_action(action),
        Some(Commands::Download { repo_id, filename }) => {
            let client = reqwest::Client::new();
            let res = client.post("http://localhost:5556/api/models/download")
                .json(&serde_json::json!({ "repo_id": repo_id, "filename": filename }))
                .send()
                .await?;
            println!("Download request sent: {}", res.text().await?);
            Ok(())
        }
        None => {
            // Default action: start the web server (Phase 5)
            println!("Starting web server (stub)...");
            Ok(())
        }
    }
}

fn handle_service_action(action: ServiceAction) -> Result<()> {
    match action {
        ServiceAction::Install => install_service(),
        ServiceAction::Start => run_systemctl("start"),
        ServiceAction::Stop => run_systemctl("stop"),
        ServiceAction::Status => run_systemctl("status"),
        ServiceAction::Log => tail_logs(),
    }
}

fn install_service() -> Result<()> {
    let exe_path = std::env::current_exe()
        .context("Failed to get current executable path")?;
    let home_dir = std::env::var("HOME").context("HOME env var not set")?;
    let service_dir = format!("{}/.config/systemd/user", home_dir);
    std::fs::create_dir_all(&service_dir)?;
    
    let service_path = format!("{}/backyard.service", service_dir);
    let service_content = format!(
r#"[Unit]
Description=Backyard Backend Service
After=network.target

[Service]
ExecStart={}
WorkingDirectory={}
Restart=always

[Install]
WantedBy=default.target
"#, exe_path.display(), std::env::current_dir()?.display());

    std::fs::write(&service_path, service_content)?;
    println!("Service file installed at {}", service_path);
    
    Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
        
    Ok(())
}

fn run_systemctl(action: &str) -> Result<()> {
    let status = Command::new("systemctl")
        .args(["--user", action, "backyard.service"])
        .status()
        .context(format!("Failed to execute systemctl {}", action))?;
        
    if !status.success() {
        anyhow::bail!("systemctl {} failed", action);
    }
    Ok(())
}

fn tail_logs() -> Result<()> {
    Command::new("journalctl")
        .args(["--user", "-u", "backyard.service", "-f"])
        .status()
        .context("Failed to execute journalctl")?;
    Ok(())
}
