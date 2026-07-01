use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use dioxus::prelude::*;

const DEFAULT_STYLES: &str = include_str!("../../../assets/tailwind.css");

#[derive(Clone, Debug)]
pub struct RenderSnapshotOptions {
    pub name: String,
    pub output_dir: PathBuf,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub device_scale_factor: f32,
    pub root_class: String,
    pub css: String,
    pub chrome_path: Option<PathBuf>,
}

impl RenderSnapshotOptions {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn with_output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = output_dir.into();
        self
    }

    pub fn with_viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport_width = width;
        self.viewport_height = height;
        self
    }

    pub fn with_root_class(mut self, root_class: impl Into<String>) -> Self {
        self.root_class = root_class.into();
        self
    }

    pub fn with_css(mut self, css: impl Into<String>) -> Self {
        self.css = css.into();
        self
    }

    pub fn with_chrome_path(mut self, chrome_path: impl Into<PathBuf>) -> Self {
        self.chrome_path = Some(chrome_path.into());
        self
    }
}

impl Default for RenderSnapshotOptions {
    fn default() -> Self {
        let output_dir = std::env::var_os("XIV_RENDER_SNAPSHOT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("target")
                    .join("render-snapshots")
            });

        Self {
            name: "snapshot".to_string(),
            output_dir,
            viewport_width: 1280,
            viewport_height: 900,
            device_scale_factor: 1.0,
            root_class: "min-h-screen bg-background text-foreground p-6".to_string(),
            css: DEFAULT_STYLES.to_string(),
            chrome_path: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderSnapshot {
    pub html_path: PathBuf,
    pub png_path: PathBuf,
}

#[derive(Debug)]
pub enum RenderSnapshotError {
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    ChromeNotFound,
    ChromeFailed {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    MissingScreenshot(PathBuf),
}

impl fmt::Display for RenderSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} {}: {source}", path.display()),
            Self::ChromeNotFound => write!(
                f,
                "could not find Chrome/Edge; set XIV_RENDER_CHROME to an executable path"
            ),
            Self::ChromeFailed {
                status,
                stdout,
                stderr,
            } => write!(
                f,
                "Chrome screenshot failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
                status, stdout, stderr
            ),
            Self::MissingScreenshot(path) => {
                write!(f, "Chrome completed but did not create {}", path.display())
            }
        }
    }
}

impl std::error::Error for RenderSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct SnapshotRootProps<T: Clone + 'static> {
    model: T,
    render: Arc<dyn Fn(T) -> Element>,
}

fn snapshot_root<T: Clone + 'static>(props: SnapshotRootProps<T>) -> Element {
    (props.render)(props.model)
}

pub fn render_snapshot(
    name: impl Into<String>,
    render: impl Fn() -> Element + 'static,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    render_snapshot_with_options(RenderSnapshotOptions::new(name), render)
}

pub fn render_snapshot_with_options(
    options: RenderSnapshotOptions,
    render: impl Fn() -> Element + 'static,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    render_model_snapshot_with_options(options, (), move |()| render())
}

pub fn render_model_snapshot<T: Clone + 'static>(
    name: impl Into<String>,
    model: T,
    render: impl Fn(T) -> Element + 'static,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    render_model_snapshot_with_options(RenderSnapshotOptions::new(name), model, render)
}

pub fn render_model_snapshot_with_options<T: Clone + 'static>(
    options: RenderSnapshotOptions,
    model: T,
    render: impl Fn(T) -> Element + 'static,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    let props = SnapshotRootProps {
        model,
        render: Arc::new(render),
    };
    let mut dom = VirtualDom::new_with_props(snapshot_root::<T>, props);
    dom.rebuild_in_place();
    write_snapshot(options, dioxus_ssr::render(&dom))
}

pub fn render_element_snapshot(
    name: impl Into<String>,
    element: Element,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    render_element_snapshot_with_options(RenderSnapshotOptions::new(name), element)
}

pub fn render_element_snapshot_with_options(
    options: RenderSnapshotOptions,
    element: Element,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    write_snapshot(options, dioxus_ssr::render_element(element))
}

fn write_snapshot(
    options: RenderSnapshotOptions,
    rendered_body: String,
) -> Result<RenderSnapshot, RenderSnapshotError> {
    let name = sanitize_file_stem(&options.name);
    fs::create_dir_all(&options.output_dir).map_err(|source| RenderSnapshotError::Io {
        action: "create output directory",
        path: options.output_dir.clone(),
        source,
    })?;

    let html_path = options.output_dir.join(format!("{name}.html"));
    let png_path = options.output_dir.join(format!("{name}.png"));
    let html = snapshot_html(&options, rendered_body);
    fs::write(&html_path, html).map_err(|source| RenderSnapshotError::Io {
        action: "write HTML snapshot",
        path: html_path.clone(),
        source,
    })?;

    let chrome = options
        .chrome_path
        .clone()
        .or_else(find_chrome)
        .ok_or(RenderSnapshotError::ChromeNotFound)?;
    render_html_with_chrome(&chrome, &html_path, &png_path, &options)?;

    Ok(RenderSnapshot {
        html_path,
        png_path,
    })
}

fn snapshot_html(options: &RenderSnapshotOptions, body: String) -> String {
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>{}</style>
  </head>
  <body>
    <main id="snapshot-root" class="{}">{}</main>
  </body>
</html>
"#,
        options.css, options.root_class, body
    )
}

fn render_html_with_chrome(
    chrome: &Path,
    html_path: &Path,
    png_path: &Path,
    options: &RenderSnapshotOptions,
) -> Result<(), RenderSnapshotError> {
    let profile_dir = options.output_dir.join(format!(
        ".chrome-profile-{}",
        sanitize_file_stem(&options.name)
    ));
    let _ = fs::remove_dir_all(&profile_dir);
    fs::create_dir_all(&profile_dir).map_err(|source| RenderSnapshotError::Io {
        action: "create Chrome profile directory",
        path: profile_dir.clone(),
        source,
    })?;

    let url = file_url(html_path);
    let output = Command::new(chrome)
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        .arg(format!(
            "--window-size={},{}",
            options.viewport_width, options.viewport_height
        ))
        .arg(format!(
            "--force-device-scale-factor={}",
            options.device_scale_factor
        ))
        .arg(format!("--screenshot={}", png_path.display()))
        .arg(OsString::from(url))
        .output()
        .map_err(|source| RenderSnapshotError::Io {
            action: "run Chrome",
            path: chrome.to_path_buf(),
            source,
        })?;

    let _ = fs::remove_dir_all(&profile_dir);

    if !output.status.success() {
        return Err(RenderSnapshotError::ChromeFailed {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    if !png_path.exists() {
        return Err(RenderSnapshotError::MissingScreenshot(
            png_path.to_path_buf(),
        ));
    }

    Ok(())
}

fn find_chrome() -> Option<PathBuf> {
    for key in ["XIV_RENDER_CHROME", "CHROME", "CHROME_PATH"] {
        if let Some(path) = std::env::var_os(key).map(PathBuf::from) {
            if path.exists() {
                return Some(path);
            }
        }
    }

    chrome_candidates().into_iter().find(|path| path.exists())
}

#[cfg(target_os = "windows")]
fn chrome_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
            candidates.push(base.join("Google/Chrome/Application/chrome.exe"));
            candidates.push(base.join("Microsoft/Edge/Application/msedge.exe"));
        }
    }
    candidates
}

#[cfg(target_os = "macos")]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into(),
    ]
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        "google-chrome".into(),
        "google-chrome-stable".into(),
        "chromium".into(),
        "chromium-browser".into(),
        "microsoft-edge".into(),
    ]
}

fn sanitize_file_stem(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            result.push(ch);
        } else {
            result.push('-');
        }
    }

    let result = result.trim_matches('-');
    if result.is_empty() {
        "snapshot".to_string()
    } else {
        result.to_string()
    }
}

fn file_url(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut absolute = absolute.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = absolute.strip_prefix("//?/") {
        absolute = stripped.to_string();
    }
    format!("file:///{}", percent_encode_url_path(&absolute))
}

fn percent_encode_url_path(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | ':' | '.' | '-' | '_' | '~') {
            result.push(ch);
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}
