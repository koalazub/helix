use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Kernel process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInfo {
    pub kernel_id: u32,
    pub lang: String,
    pub kernel_dir: PathBuf,
    pub input_file: PathBuf,
    pub output_file: PathBuf,
    pub pid: u32,
}

/// Manages a single kernel process
pub struct KernelManager {
    kernel_dir: PathBuf,
    lang: String,
}

impl KernelManager {
    /// Create a new kernel manager for a specific language
    pub fn new(lang: impl AsRef<str>, kernel_id: u32) -> Self {
        let kernel_dir = PathBuf::from(format!("/tmp/helix-kernel-{}", kernel_id));
        Self {
            kernel_dir,
            lang: lang.as_ref().to_string(),
        }
    }

    /// Start a new kernel process (kills any existing kernel with the same ID)
    pub async fn start(&self) -> Result<KernelInfo> {
        // Kill any existing kernel
        self.cleanup().await?;

        // Create kernel directory
        fs::create_dir_all(&self.kernel_dir)
            .await
            .context("Failed to create kernel directory")?;

        let input_file = self.kernel_dir.join("input.jl");
        let output_file = self.kernel_dir.join("output.txt");
        let runner_file = self.kernel_dir.join("runner.jl");
        let log_file = self.kernel_dir.join("kernel.log");

        // Write runner script
        let runner_script = self.create_runner_script(&input_file, &output_file)?;
        fs::write(&runner_file, runner_script)
            .await
            .context("Failed to write runner script")?;

        // Start Julia process
        let child = Command::new("julia")
            .arg(&runner_file)
            .stdout(std::process::Stdio::null())  // Suppress stdout - kernel writes to file
            .stderr(std::process::Stdio::null())  // Suppress stderr
            .spawn()
            .context("Failed to start Julia process")?;

        let pid = child.id().context("Failed to get process ID")?;

        // Write PID file
        let pid_file = self.kernel_dir.join("pid");
        fs::write(&pid_file, pid.to_string())
            .await
            .context("Failed to write PID file")?;

        // Wait a moment for kernel to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        Ok(KernelInfo {
            kernel_id: 1,
            lang: self.lang.clone(),
            kernel_dir: self.kernel_dir.clone(),
            input_file,
            output_file,
            pid,
        })
    }

    /// Create the Julia runner script
    fn create_runner_script(&self, input_file: &Path, output_file: &Path) -> Result<String> {
        let input_path = input_file
            .to_str()
            .context("Invalid input file path")?;
        let output_path = output_file
            .to_str()
            .context("Invalid output file path")?;

        Ok(format!(
            r#"# Helix Kernel Runner - REPL-style
const input_file = "{}"
const output_file = "{}"
const marker = "__COMPLETE__"

println(stderr, "Kernel ready (REPL mode)")

while true
    if isfile(input_file)
        code = read(input_file, String)
        try
            rm(input_file)
        catch
            # Input file already deleted, continue
        end

        # Clear output and remove completion flag
        write(output_file, "")
        flush(stdout)
        isfile(output_file * ".done") && rm(output_file * ".done")

        # Execute and capture output like REPL
        open(output_file, "w") do io
            # Redirect both stdout and stderr
            redirect_stdout(io) do
                redirect_stderr(io) do
                    try
                        # Parse and evaluate code
                        result = include_string(Main, code)

                        # Print result if not nothing (REPL behavior)
                        if result !== nothing
                            println(io, result)
                        end
                    catch e
                        showerror(io, e, catch_backtrace())
                    end
                end
            end
        end

        # Append completion marker (with newline separator)
        open(output_file, "a") do io
            println(io)  # Ensure newline before marker
            println(io, marker)
        end

        # Create completion flag file for event-based notification
        touch(output_file * ".done")
    end
    sleep(0.1)
end
"#,
            input_path, output_path
        ))
    }

    /// Clean up any existing kernel (kill process, remove directory)
    async fn cleanup(&self) -> Result<()> {
        let pid_file = self.kernel_dir.join("pid");

        // Try to kill existing process
        if let Ok(pid_str) = fs::read_to_string(&pid_file).await {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
                #[cfg(windows)]
                {
                    // Windows process termination
                    use std::process::Command as StdCommand;
                    let _ = StdCommand::new("taskkill")
                        .args(["/PID", &pid.to_string(), "/F"])
                        .output();
                }
            }
        }

        // Remove kernel directory
        if self.kernel_dir.exists() {
            fs::remove_dir_all(&self.kernel_dir)
                .await
                .context("Failed to remove kernel directory")?;
        }

        Ok(())
    }

    /// Execute code in the kernel
    pub async fn execute(&self, code: impl AsRef<str>) -> Result<String> {
        let input_file = self.kernel_dir.join("input.jl");
        let output_file = self.kernel_dir.join("output.txt");
        let done_file = self.kernel_dir.join("output.txt.done");

        // Clear any stale done file
        let _ = fs::remove_file(&done_file).await;

        // Write code to input file
        fs::write(&input_file, code.as_ref())
            .await
            .context("Failed to write input file")?;

        // Wait for completion (poll for .done file)
        let timeout = tokio::time::Duration::from_secs(30);
        let start = tokio::time::Instant::now();

        while !done_file.exists() {
            if start.elapsed() > timeout {
                anyhow::bail!("Kernel execution timeout");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Read output
        let output = fs::read_to_string(&output_file)
            .await
            .context("Failed to read output file")?;

        // Filter out marker
        let filtered: Vec<&str> = output
            .lines()
            .filter(|line| *line != "__COMPLETE__")
            .collect();

        Ok(filtered.join("\n"))
    }

    /// Stop the kernel
    pub async fn stop(&self) -> Result<()> {
        self.cleanup().await
    }
}

// Synchronous wrappers for Steel integration
/// Start a kernel (synchronous wrapper)
pub fn start_kernel_sync(lang: &str, kernel_id: u32) -> Result<KernelInfo> {
    let rt = tokio::runtime::Runtime::new()?;
    let manager = KernelManager::new(lang, kernel_id);
    rt.block_on(manager.start())
}

/// Execute code in kernel (synchronous wrapper)
pub fn execute_kernel_sync(kernel_id: u32, code: &str) -> Result<String> {
    let rt = tokio::runtime::Runtime::new()?;
    let manager = KernelManager::new("julia", kernel_id);
    rt.block_on(manager.execute(code))
}

/// Stop kernel (synchronous wrapper)
pub fn stop_kernel_sync(kernel_id: u32) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let manager = KernelManager::new("julia", kernel_id);
    rt.block_on(manager.stop())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kernel_lifecycle() {
        let manager = KernelManager::new("julia", 999);

        // Start kernel
        let info = manager.start().await.unwrap();
        assert_eq!(info.lang, "julia");
        assert!(info.kernel_dir.exists());

        // Execute code
        let result = manager.execute("2 + 2").await.unwrap();
        assert!(result.contains("4"));

        // Stop kernel
        manager.stop().await.unwrap();
        assert!(!info.kernel_dir.exists());
    }
}
