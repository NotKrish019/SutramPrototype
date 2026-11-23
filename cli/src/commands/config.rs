//! `yukti config`: the resolved configuration and where each value came from. No network.

use serde::Serialize;

use crate::cli::{Cli, ENV_CASE};
use crate::config::{Config, EnvVars, ValueSource, ENV_API_URL, ENV_TIMEOUT_SECS};
use crate::error::exit;
use crate::output::{to_json, Rendered};
use crate::ui::{ColorLevel, Glyph, Tone, Ui};

pub const CONFIG_SCHEMA: &str = "yukti.cli.config/v1";

#[derive(Serialize)]
struct Setting<'a> {
    value: String,
    source: &'a str,
}

#[derive(Serialize)]
struct ConfigDocument<'a> {
    schema: &'static str,
    version: &'static str,
    api_url: Setting<'a>,
    timeout_secs: Setting<'a>,
    /// `YUKTI_CASE`, used for `--case` when the option is omitted.
    working_case: Option<&'a str>,
    color: &'static str,
    unicode: bool,
    width: usize,
}

fn source_label(source: ValueSource, env: &'static str, flag: &'static str) -> &'static str {
    match source {
        ValueSource::Flag => flag,
        ValueSource::Env => env,
        ValueSource::Default => "default",
    }
}

fn color_label(level: ColorLevel) -> &'static str {
    match level {
        ColorLevel::TrueColor => "truecolor",
        ColorLevel::Ansi256 => "256",
        ColorLevel::None => "none",
    }
}

pub fn run(config: &Config, env: &EnvVars, cli: &Cli, ui: &Ui) -> Rendered {
    let api_url = Setting {
        value: config.base_url(),
        source: source_label(config.api_url_source, ENV_API_URL, "--api-url"),
    };
    let timeout = Setting {
        value: config.timeout.as_secs().to_string(),
        source: source_label(config.timeout_source, ENV_TIMEOUT_SECS, "--timeout"),
    };
    let case = env
        .case_id
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if cli.json {
        let doc = ConfigDocument {
            schema: CONFIG_SCHEMA,
            version: env!("CARGO_PKG_VERSION"),
            api_url,
            timeout_secs: timeout,
            working_case: case,
            color: color_label(ui.color),
            unicode: ui.unicode,
            width: ui.width,
        };
        return Rendered::stdout(to_json(&doc), exit::SUCCESS);
    }
    let src = |s: &str| ui.paint(Tone::Subtle, &format!("  {} {s}", ui.g(Glyph::Dot)));
    let mut out = ui.header(&["config"], &format!("v{}", env!("CARGO_PKG_VERSION")));
    out.push_str(&ui.section("Connection", "flag > environment > default"));
    out.push_str(&ui.kv_painted(
        "API URL",
        &format!(
            "{}{}",
            ui.paint(Tone::Text, &api_url.value),
            src(api_url.source)
        ),
    ));
    out.push_str(&ui.kv_painted(
        "Timeout",
        &format!(
            "{}{}",
            ui.paint(Tone::Text, &format!("{} s", timeout.value)),
            src(timeout.source)
        ),
    ));
    out.push_str(&ui.section("Working case", ENV_CASE));
    match case {
        Some(id) => out.push_str(&ui.kv_painted(
            "Case",
            &format!("{}{}", ui.paint(Tone::Text, id), src(ENV_CASE)),
        )),
        None => {
            out.push_str(&ui.kv("Case", Tone::Muted, "not set: commands need --case"));
            out.push_str(&ui.hint(&format!(
                "export {ENV_CASE}=<uuid> to skip --case on every command"
            )));
        }
    }
    out.push_str(&ui.section("Display", ""));
    out.push_str(&ui.kv("Colour", Tone::Text, color_label(ui.color)));
    out.push_str(&ui.kv(
        "Glyphs",
        Tone::Text,
        if ui.unicode { "unicode" } else { "ascii" },
    ));
    out.push_str(&ui.kv("Width", Tone::Text, &format!("{} columns", ui.width)));
    out.push('\n');
    out.push_str(&ui.legend());
    Rendered::stdout(out, exit::SUCCESS)
}
