use std::ffi::OsStr;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

pub fn spawn_image_viewer(
    paths: &[impl AsRef<OsStr>],
    class: &str,
    fullscreen: bool,
) -> Result<()> {
    let mut command = Command::new("nsxiv");
    command.arg("--class").arg(class);
    if fullscreen {
        command.args([
            "--fullscreen",
            "--scale-mode",
            "f", // fit
        ]);
    }
    command
        .args(paths)
        .spawn()
        .with_context(|| "Spawning image viewer")?;
    Ok(())
}

pub fn kill_process_class(class: &str) -> Result<()> {
    Command::new("pkill")
        .arg("--full")
        .arg(class)
        .status()
        .with_context(|| "Killing image viewer")?;
    Ok(())
}

pub fn open_editor(path: impl AsRef<OsStr>) -> Result<()> {
    Command::new("nvim")
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| "Opening editor")?;
    Ok(())
}

/// BSPWM-specific functionality
pub fn setup_image_viewer_window(paths: &[impl AsRef<OsStr>], viewer_class: &str) -> Result<()> {
    // Window ID of main window (terminal)
    let bspc_node = bspc_command(&["query", "-N", "-n"])?.stdout;
    let bspc_node = std::str::from_utf8(&bspc_node)
        .expect("commmand result should be utf-8")
        .trim();

    // Temporary hide currently focused window
    // To avoid attaching image viewer to `tabbed` instance
    bspc_command(&["node", bspc_node, "-g", "hidden"])?;

    spawn_image_viewer(paths, viewer_class, false)?;
    // Wait for image viewer to completely start
    thread::sleep(Duration::from_millis(50));

    // Unhide main window
    // Move image viewer to left, resize slightly, re-focus main window
    bspc_command(&["node", bspc_node, "-g", "hidden"])?;
    bspc_command(&["node", "-s", "west"])?;
    bspc_command(&["node", "-z", "right", "-200", "0"])?;
    bspc_command(&["node", "-f", "east"])?;

    Ok(())
}

fn bspc_command(args: &[impl AsRef<OsStr>]) -> Result<process::Output> {
    let output = Command::new("bspc").args(args).output().with_context(|| {
        format!(
            "Run `bspc` command {:?}",
            // TODO(refactor): Extract this line to a function
            args.into_iter().map(|x| x.as_ref()).collect::<Vec<_>>()
        )
    })?;
    Ok(output)
}
