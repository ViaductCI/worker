use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::process::Command;
use std::fs;
use colored::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Job {
    name: String,
    repository: String,
    branch: String,
    commands: Vec<String>,
    #[serde(default)]
    inputs: Vec<JobInput>,
    #[serde(default)]
    outputs: Vec<JobOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobInput {
    name: String,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JobOutput {
    name: String,
    path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JobResult {
    id: String,
    status: String,
    output: String,
    artifacts: Vec<JobArtifact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JobArtifact {
    name: String,
    content: String,
}

async fn process_job(job: web::Json<Job>) -> impl Responder {
    println!("{} Received job: {:?}", "🛎️".green(), job);
    
    let result = execute_job(&job);
    
    println!("{} Sending result: {:?}", "📤".blue(), result);
    
    HttpResponse::Ok().json(result)
}

fn execute_job(job: &Job) -> JobResult {
    let work_dir = format!("work_{}", Uuid::new_v4());
    fs::create_dir(&work_dir).expect(&format!("{} Failed to create work directory", "❌".red()));

    let mut output = String::new();
    let mut status = "success".to_string();
    let mut artifacts = Vec::new();

    // Clone repository
    println!("{} Cloning repository: {}", "🔄".yellow(), job.repository);
    let clone_result = Command::new("git")
        .args(&["clone", "-b", &job.branch, &job.repository, &work_dir])
        .output();

    match clone_result {
        Ok(clone_output) => {
            if !clone_output.status.success() {
                status = "failed".to_string();
                output = format!("{} Failed to clone repository: {}", "❌".red(), String::from_utf8_lossy(&clone_output.stderr));
            } else {
                println!("{} Repository cloned successfully", "✅".green());
            }
        }
        Err(e) => {
            status = "failed".to_string();
            output = format!("{} Error cloning repository: {}", "❌".red(), e);
        }
    }

    // Execute commands if cloning was successful
    if status == "success" {
        for (i, cmd) in job.commands.iter().enumerate() {
            println!("{} Executing command {}/{}: {}", "🚀".cyan(), i+1, job.commands.len(), cmd);
            let cmd_result = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&work_dir)
                .output();

            match cmd_result {
                Ok(cmd_output) => {
                    output.push_str(&format!("{} Command: {}\n", "🖥️".blue(), cmd));
                    output.push_str(&String::from_utf8_lossy(&cmd_output.stdout));
                    output.push_str(&String::from_utf8_lossy(&cmd_output.stderr));
                    
                    if !cmd_output.status.success() {
                        status = "failed".to_string();
                        println!("{} Command failed", "❌".red());
                        break;
                    } else {
                        println!("{} Command executed successfully", "✅".green());
                    }
                }
                Err(e) => {
                    status = "failed".to_string();
                    output.push_str(&format!("{} Error executing command: {}\n", "❌".red(), e));
                    println!("{} Error executing command: {}", "❌".red(), e);
                    break;
                }
            }
        }

        // Collect artifacts
        println!("{} Collecting artifacts", "📦".magenta());
        for output_spec in &job.outputs {
            let path = format!("{}/{}", work_dir, output_spec.path);
            match fs::read_to_string(&path) {
                Ok(content) => {
                    artifacts.push(JobArtifact {
                        name: output_spec.name.clone(),
                        content,
                    });
                    println!("{} Artifact collected: {}", "✅".green(), output_spec.name);
                }
                Err(e) => {
                    output.push_str(&format!("{} Error reading output {}: {}\n", "❌".red(), output_spec.name, e));
                    println!("{} Error reading artifact {}: {}", "❌".red(), output_spec.name, e);
                }
            }
        }
    }

    // Cleanup
    println!("{} Cleaning up work directory", "🧹".yellow());
    fs::remove_dir_all(&work_dir).expect(&format!("{} Failed to remove work directory", "❌".red()));

    let result = JobResult {
        id: Uuid::new_v4().to_string(),
        status: status.clone(),
        output,
        artifacts,
    };

    if status == "success" {
        println!("{} Job completed successfully", "🎉".green());
    } else {
        println!("{} Job failed", "💔".red());
    }

    result
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("{} Starting CI/CD worker server", "🚀".green());
    HttpServer::new(|| {
        App::new()
            .route("/job", web::post().to(process_job))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
