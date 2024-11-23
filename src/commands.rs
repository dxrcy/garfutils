use std::ffi::OsStr;
use std::fmt::Write as _;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

pub fn spawn_image_viewer(paths: &[impl AsRef<OsStr>], name: &str, fullscreen: bool) -> Result<()> {
    let mut command = Command::new("swiv");
    if fullscreen {
        command.args([
            "-f", // Fullscreen
            "-s", "f", // Scale mode: fit
        ]);
    }
    command
        .args(["-N", name]) // Window name (so it can be killed later)
        .args(["-B", "#000000"]) // Background color
        .args(paths)
        .spawn()
        .with_context(|| "Spawning image viewer")?;
    Ok(())
}

pub fn kill_process_name(name: &str) -> Result<()> {
    Command::new("pkill")
        .arg("--full")
        .arg(name)
        .status()
        .with_context(|| "Killing image viewer")?;
    // Non-zero exit means no process found; not necessarily an error
    Ok(())
}

pub fn open_editor(path: impl AsRef<OsStr>) -> Result<()> {
    let status = Command::new("nvim")
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| "Opening editor")?;
    if !status.success() {
        bail!("Editor did not exit successfully");
    }
    Ok(())
}

/// Hyprland-specific functionality
pub fn setup_image_viewer_window(paths: &[impl AsRef<OsStr>], window_name: &str) -> Result<()> {
    spawn_image_viewer(paths, window_name, false)?;

    // Wait for image viewer to completely start
    // TODO(fix): Spin until image viewer window has spawned
    thread::sleep(Duration::from_millis(200));

    // Move image viewer to left, resize slightly, re-focus main window
    hyprctl_command(&["moveoutofgroup"])?;
    hyprctl_command(&["swapwindow", "l"])?;
    hyprctl_command(&["resizeactive", "-250", "0"])?;
    hyprctl_command(&["movefocus", "r"])?;

    Ok(())
}

fn hyprctl_command(args: &[impl AsRef<OsStr>]) -> Result<process::Output> {
    let output = Command::new("hyprctl")
        .arg("dispatch")
        .args(args)
        .output()
        .with_context(|| format!("Run command `hyprctl dispatch {}`", stringify_args(args)))?;
    if !output.status.success() {
        bail!(
            "Command did not exit successfully: `hyprctl dispatch {}`",
            stringify_args(args)
        );
    }
    Ok(output)
}

fn stringify_args(args: &[impl AsRef<OsStr>]) -> String {
    let mut output = String::new();
    for arg in args {
        if !output.is_empty() {
            output += " ";
        }
        write!(output, "{:?}", arg.as_ref()).expect("write to string should not fail");
    }
    output
}
