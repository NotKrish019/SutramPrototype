//! `yukti open`: hand the investigation from the terminal to the web workspace.
//!
//! The CLI and the browser workspace are two clients of the same API, so a case worked on here is
//! already in the web. This command builds the workspace URL for a case and view, checks the case
//! exists, and opens it in the default browser when stdout is an interactive terminal.

use clap::ValueEnum;
use reqwest::Url;
use serde::Serialize;

use crate::cli::OpenArgs;
use crate::client::ApiClient;
use crate::config::EnvVars;
use crate::error::CliError;
use crate::output::{render_error, to_json, Rendered};
use crate::ui::{Glyph, Tone, Ui, MARGIN};

/// Default address of the web workspace (Vite's dev server in `apps/web/vite.config.ts`).
pub const DEFAULT_WEB_URL: &str = "http://127.0.0.1:5173";
/// Environment variable for the web workspace address.
pub const ENV_WEB_URL: &str = "YUKTI_WEB_URL";
/// Schema of the `--json` document.
pub const OPEN_SCHEMA: &str = "yukti.cli.open/v1";

/// A view of the web workspace (`/workspace/cases/<id>/<view>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WebView {
    /// What happened, what conflicts, what to do next
    Overview,
    /// Animated, narrated replay of the whole reconstruction
    Theatre,
    /// Exhibits, hashes and chain of custody
    Evidence,
    /// Every source on one corrected clock
    Timeline,
    /// Where the evidence places each event
    Spatial,
    /// Claims the evidence cannot reconcile
    Contradictions,
    /// Keyframes and frame provenance
    Video,
    /// Judicial report
    Reports,
    /// Knowledge graph
    Graph,
    /// Which exhibits support each other
    Correlation,
}

impl WebView {
    fn segment(self) -> Option<&'static str> {
        Some(match self {
            Self::Overview => return None,
            Self::Theatre => "theatre",
            Self::Evidence => "evidence",
            Self::Timeline => "timeline",
            Self::Spatial => "spatial",
            Self::Contradictions => "contradictions",
            Self::Video => "video",
            Self::Reports => "reports",
            Self::Graph => "graph",
            Self::Correlation => "correlation",
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Investigation overview",
            Self::Theatre => "Reconstruction theatre",
            Self::Evidence => "Evidence intake",
            Self::Timeline => "Temporal reconstruction",
            Self::Spatial => "Spatial reconstruction",
            Self::Contradictions => "Contradiction intelligence",
            Self::Video => "Video intelligence",
            Self::Reports => "Judicial report",
            Self::Graph => "Knowledge graph",
            Self::Correlation => "Evidence correlation",
        }
    }
}

/// Resolves the web workspace address: `--web-url`, then `YUKTI_WEB_URL`, then the default.
pub fn resolve_web_url(flag: Option<&str>, env: &EnvVars) -> Result<Url, CliError> {
    let raw = flag
        .map(str::to_string)
        .or_else(|| env.web_url.clone())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_WEB_URL.to_string());
    let url = Url::parse(raw.trim()).map_err(|e| {
        CliError::InvalidArgument(format!("web URL {raw:?} is not a valid URL ({e})"))
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(CliError::InvalidArgument(format!(
            "web URL {raw:?} must use http or https"
        )));
    }
    Ok(url)
}

/// The workspace URL for a case and view, or the workspace home without a case.
pub fn workspace_url(base: &Url, case_id: Option<&str>, view: WebView) -> String {
    let root = base.as_str().trim_end_matches('/');
    match case_id {
        None => format!("{root}/workspace"),
        Some(id) => match view.segment() {
            None => format!("{root}/workspace/cases/{id}"),
            Some(seg) => format!("{root}/workspace/cases/{id}/{seg}"),
        },
    }
}

/// Opens `url` in the default browser. Returns false if no launcher could be started.
fn launch_browser(url: &str) -> bool {
    use std::process::{Command, Stdio};
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        // The empty string is the window title `start` expects before a quoted target.
        c.args(["/C", "start", "", url]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

#[derive(Serialize)]
struct OpenDocument<'a> {
    schema: &'static str,
    url: &'a str,
    api_url: &'a str,
    case_id: Option<&'a str>,
    case_title: Option<&'a str>,
    view: WebView,
    launched: bool,
}

pub async fn run(
    args: &OpenArgs,
    env: &EnvVars,
    client: &ApiClient,
    json: bool,
    ui: &Ui,
) -> Rendered {
    let base = match resolve_web_url(args.web_url.as_deref(), env) {
        Ok(url) => url,
        Err(err) => return render_error(&err, json),
    };
    let case = match args.case_id.as_deref() {
        Some(raw) => match super::require_case(raw, client, json, "opened").await {
            Ok((id, record)) => Some((id, record)),
            Err(rendered) => return rendered,
        },
        None => None,
    };
    let case_id = case.as_ref().map(|(id, _)| id.as_str().to_string());
    let case_title = case.as_ref().map(|(_, r)| r.dto.title.clone());
    let view = if case.is_some() {
        args.view
    } else {
        WebView::Overview
    };
    let url = workspace_url(&base, case_id.as_deref(), view);
    let api_url = client.base_url().to_string();

    let launched = !args.print && !json && env.terminal.stdout_tty && launch_browser(&url);

    if json {
        let doc = OpenDocument {
            schema: OPEN_SCHEMA,
            url: &url,
            api_url: api_url.trim_end_matches('/'),
            case_id: case_id.as_deref(),
            case_title: case_title.as_deref(),
            view,
            launched,
        };
        return Rendered::stdout(to_json(&doc), 0);
    }

    let mut out = String::new();
    out.push_str(&ui.header(&["open"], base.as_str().trim_end_matches('/')));
    out.push('\n');
    let status = if launched {
        format!(
            "{} {}",
            ui.mark(Glyph::Check, Tone::Ok),
            ui.bold(Tone::Ok, "OPENED IN YOUR BROWSER")
        )
    } else {
        format!(
            "{} {}",
            ui.mark(Glyph::Arrow, Tone::Brand),
            ui.bold(Tone::Strong, "OPEN THIS LINK")
        )
    };
    out.push_str(&format!("{MARGIN}{status}\n"));
    out.push_str(&format!(
        "{MARGIN}{}\n",
        ui.link(&url, &ui.bold(Tone::Brand, &url))
    ));
    out.push('\n');
    if let Some(title) = &case_title {
        out.push_str(&ui.kv("Case", Tone::Strong, title));
    }
    out.push_str(&ui.kv("View", Tone::Text, view.label()));
    out.push_str(&ui.kv(
        "Shared API",
        Tone::Muted,
        &format!(
            "{}  (web and CLI read the same records)",
            api_url.trim_end_matches('/')
        ),
    ));
    out.push('\n');
    if let Some(id) = &case_id {
        if view != WebView::Theatre {
            out.push_str(&ui.next(
                "watch it",
                &format!("yukti open --case {id} --view theatre"),
            ));
        }
        out.push_str(&ui.next("replay", &format!("yukti replay --case {id}")));
    } else {
        out.push_str(&ui.next("a case", "yukti open --case <uuid> --view timeline"));
    }
    if !launched && !args.print {
        out.push_str(&ui.footnote(&format!(
            "Start the web workspace with `npm run dev` in apps/web, or set {ENV_WEB_URL} to where it runs."
        )));
    }
    Rendered::stdout(out, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_workspace_urls_for_every_view() {
        let base = Url::parse("http://127.0.0.1:5173/").unwrap();
        assert_eq!(
            workspace_url(&base, None, WebView::Theatre),
            "http://127.0.0.1:5173/workspace"
        );
        assert_eq!(
            workspace_url(&base, Some("abc"), WebView::Overview),
            "http://127.0.0.1:5173/workspace/cases/abc"
        );
        assert_eq!(
            workspace_url(&base, Some("abc"), WebView::Theatre),
            "http://127.0.0.1:5173/workspace/cases/abc/theatre"
        );
    }

    #[test]
    fn web_url_precedence_and_validation() {
        let env = EnvVars {
            web_url: Some("https://yukti.example:8443".into()),
            ..EnvVars::default()
        };
        assert_eq!(
            resolve_web_url(None, &env).unwrap().as_str(),
            "https://yukti.example:8443/"
        );
        assert_eq!(
            resolve_web_url(Some("http://localhost:4173"), &env)
                .unwrap()
                .as_str(),
            "http://localhost:4173/"
        );
        assert_eq!(
            resolve_web_url(None, &EnvVars::default()).unwrap().as_str(),
            "http://127.0.0.1:5173/"
        );
        assert!(resolve_web_url(Some("ftp://x"), &env).is_err());
        assert!(resolve_web_url(Some("not a url"), &env).is_err());
    }
}
