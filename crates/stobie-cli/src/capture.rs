//! Screenshots bracketed by observations.
//!
//! A native application is captured through X11, as the Node client does:
//! `xdotool` finds the window of the launched process and ImageMagick's
//! `import` writes it. A browser page's canvas is captured with the DevTools
//! `Page.captureScreenshot`. The interface protocol has no screenshot
//! operation.

use std::path::Path;
use std::process::Command;

use serde::Serialize;
use stobie_client::{BrowserTransport, Client, ClientError, Transport};

use crate::Output;
use crate::session::Target;

/// What a capture establishes, and what it could not.
#[derive(Serialize)]
pub struct Capture {
    pub path: Option<String>,
    pub route: &'static str,
    pub window: Option<String>,
    pub before_observation: Option<u64>,
    pub after_observation: Option<u64>,
    /// The screenshot and the observations are separate reads; nothing ties
    /// the image to one UI pass.
    pub exact_observation_frame: bool,
    /// Each stage that failed.
    pub diagnostics: Vec<String>,
}

pub fn capture(
    client: &mut Client<Box<dyn Transport>>,
    target: &Target,
    path: &Path,
) -> Result<Output, ClientError> {
    let mut diagnostics = Vec::new();
    let before = client.observe();
    let (route, window, written) = match target {
        Target::Native { pid, .. } => {
            let (window, written) = native(*pid, path, &mut diagnostics);
            ("native_x11", window, written)
        }
        Target::Browser {
            endpoint,
            page,
            target,
        } => {
            let selector = crate::page_selector(page.as_deref(), target.as_deref())?;
            let written = BrowserTransport::new(endpoint, selector)
                .and_then(|mut browser| {
                    browser.screenshot(std::time::Instant::now() + client.timeout())
                })
                .and_then(|png| {
                    std::fs::write(path, png).map_err(|error| {
                        ClientError::new(
                            "capture_failed",
                            format!("cannot write {}: {error}", path.display()),
                            "choose a writable --output path",
                        )
                    })
                });
            match written {
                Ok(()) => ("browser_canvas", None, Some(path.display().to_string())),
                Err(error) => {
                    diagnostics.push(format!("the screenshot failed: {}", error.message));
                    ("browser_canvas", None, None)
                }
            }
        }
    };
    let after = client.observe();
    for (stage, result) in [("before", &before), ("after", &after)] {
        if let Err(error) = result {
            diagnostics.push(format!("{stage} observation failed: {error}"));
        }
    }
    let capture = Capture {
        path: written.clone(),
        route,
        window,
        before_observation: before.as_ref().ok().map(|observation| observation.id),
        after_observation: after.as_ref().ok().map(|observation| observation.id),
        exact_observation_frame: false,
        diagnostics,
    };
    finish(capture, written)
}

/// The X11 window of process `pid`, written to `path`.
fn native(
    pid: Option<u32>,
    path: &Path,
    diagnostics: &mut Vec<String>,
) -> (Option<String>, Option<String>) {
    let window = match pid {
        Some(pid) => match Command::new("xdotool")
            .args(["search", "--onlyvisible", "--pid", &pid.to_string()])
            .output()
        {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(ToOwned::to_owned),
            Ok(_) => {
                diagnostics.push(format!("xdotool found no visible window for process {pid}"));
                None
            }
            Err(error) => {
                diagnostics.push(format!("cannot run xdotool: {error}"));
                None
            }
        },
        None => {
            diagnostics.push(
                "the window is unknown: capture needs a session started by `stobie launch` on X11"
                    .into(),
            );
            None
        }
    };
    let mut written = None;
    if let Some(window) = &window {
        match Command::new("import")
            .args(["-window", window])
            .arg(format!("png:{}", path.display()))
            .output()
        {
            Ok(output) if output.status.success() => written = Some(path.display().to_string()),
            Ok(output) => diagnostics.push(format!(
                "import failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) => diagnostics.push(format!("cannot run ImageMagick import: {error}")),
        }
    }
    (window, written)
}

fn finish(capture: Capture, written: Option<String>) -> Result<Output, ClientError> {
    if capture.path.is_none() && capture.before_observation.is_none() {
        return Err(ClientError::new(
            "capture_failed",
            capture.diagnostics.join("; "),
            "capture needs X11 with xdotool and ImageMagick, and a launched session",
        ));
    }
    let complete = capture.diagnostics.is_empty();
    let mut text = match &written {
        Some(path) => format!(
            "captured {path} between observations {} and {}\n",
            capture
                .before_observation
                .map_or("?".into(), |id| id.to_string()),
            capture
                .after_observation
                .map_or("?".into(), |id| id.to_string())
        ),
        None => "no screenshot was written\n".to_owned(),
    };
    for diagnostic in &capture.diagnostics {
        text.push_str(&format!("partial: {diagnostic}\n"));
    }
    let mut output = Output::new(text, capture);
    if !complete {
        output.status = 5;
    }
    Ok(output)
}
