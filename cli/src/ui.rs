//! YUKTI terminal design system.
//!
//! The CLI mirrors the web workspace (`apps/web/tailwind.config.js`): a matte-black
//! foundation, white type, one forensic crimson for critical intelligence and active
//! states, silver/graphite for structure, and a semantic evidence palette in which colours
//! carry meaning and are never decoration:
//!
//! | glyph | tone       | meaning                                         |
//! |-------|------------|-------------------------------------------------|
//! | `●`   | `Observed` | directly observed fact (default terminal text)  |
//! | `○`   | `Inferred` | inferred relationship or derived value (silver) |
//! | `◇`   | `Supplied` | supplied by a party or record holder (steel)    |
//! | `◆`   | `Warn`     | assumed / unverified value (amber)              |
//! | `▲`   | `Flag`     | contradiction or requires review (crimson)      |
//! | `✓`   | `Ok`       | verified / healthy (green, small use only)      |
//!
//! Every state is also spelled out in text, so nothing depends on colour alone.
//!
//! Colour is emitted only when the stream is a terminal (or `--color always`), and never
//! when `NO_COLOR` is set or `TERM=dumb`. Body text uses the terminal's own foreground so the
//! output stays readable on light themes; only accents and greys are coloured. `--ascii`
//! swaps every glyph for a 7-bit equivalent. The layout is identical in every mode.

use std::fmt::Write as _;

use clap::ValueEnum;

/// Brand palette (sRGB), copied from the web design tokens.
pub mod palette {
    pub type Rgb = (u8, u8, u8);
    /// `crimson.DEFAULT` · critical intelligence, contradictions, active state.
    pub const CRIMSON: Rgb = (0xE5, 0x24, 0x3B);
    /// `fg.muted` · labels.
    pub const MUTED: Rgb = (0xA1, 0xA1, 0xAA);
    /// `fg.subtle` · metadata, identifiers, hints.
    pub const SUBTLE: Rgb = (0x71, 0x71, 0x7A);
    /// `paper.DEFAULT` / `signal.accent` · inferred relationships.
    pub const PAPER: Rgb = (0xC8, 0xCA, 0xD0);
    /// `steel.DEFAULT` / `signal.supplied` · information supplied by a party.
    pub const STEEL: Rgb = (0x8C, 0x9E, 0xB3);
    /// `signal.ok` · verified / healthy.
    pub const OK: Rgb = (0x4F, 0xB3, 0x7D);
    /// `signal.warn` · assumed / unverified.
    pub const WARN: Rgb = (0xD9, 0xA4, 0x41);
    /// `ink.strong` · rules and borders.
    pub const LINE: Rgb = (0x3A, 0x3A, 0x41);
}

/// `--color` choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ColorChoice {
    /// Colour when writing to a terminal and NO_COLOR is not set
    #[default]
    Auto,
    /// Always emit colour (also for pipes and files)
    Always,
    /// Never emit colour
    Never,
}

/// How many colours the output stream supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorLevel {
    None,
    Ansi256,
    TrueColor,
}

/// Terminal facts captured once from the process, so rendering is deterministic in tests.
/// The default describes a non-terminal stream: no colour, 100 columns.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalEnv {
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    /// `NO_COLOR` is set to a non-empty value.
    pub no_color: bool,
    /// `CLICOLOR_FORCE` is set to a non-empty value other than `0`.
    pub force_color: bool,
    pub colorterm: Option<String>,
    pub term: Option<String>,
    /// Width of the terminal attached to stdout, when known.
    pub columns: Option<usize>,
}

impl TerminalEnv {
    /// Reads the current process's terminal and environment.
    pub fn from_process() -> Self {
        use std::io::IsTerminal;
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let columns = terminal_size::terminal_size()
            .map(|(w, _)| usize::from(w.0))
            .or_else(|| var("COLUMNS").and_then(|c| c.trim().parse().ok()));
        Self {
            stdout_tty: std::io::stdout().is_terminal(),
            stderr_tty: std::io::stderr().is_terminal(),
            no_color: var("NO_COLOR").is_some(),
            force_color: var("CLICOLOR_FORCE").is_some_and(|v| v != "0"),
            colorterm: var("COLORTERM"),
            term: var("TERM"),
            columns,
        }
    }

    fn is_dumb(&self) -> bool {
        self.term.as_deref() == Some("dumb")
    }

    fn depth(&self) -> ColorLevel {
        match self.colorterm.as_deref() {
            Some(v) if v.contains("truecolor") || v.contains("24bit") => ColorLevel::TrueColor,
            _ => ColorLevel::Ansi256,
        }
    }
}

/// Semantic tone of a piece of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Terminal default foreground: observed facts and ordinary values.
    Text,
    /// Terminal default foreground, bold: headings and key figures.
    Strong,
    /// Brand crimson, bold: the YUKTI mark and active state.
    Brand,
    Muted,
    Subtle,
    Inferred,
    Supplied,
    Ok,
    Warn,
    Flag,
    Line,
}

impl Tone {
    fn rgb(self) -> Option<palette::Rgb> {
        use palette::*;
        match self {
            Self::Text | Self::Strong => None,
            Self::Brand | Self::Flag => Some(CRIMSON),
            Self::Muted => Some(MUTED),
            Self::Subtle => Some(SUBTLE),
            Self::Inferred => Some(PAPER),
            Self::Supplied => Some(STEEL),
            Self::Ok => Some(OK),
            Self::Warn => Some(WARN),
            Self::Line => Some(LINE),
        }
    }

    fn bold(self) -> bool {
        matches!(self, Self::Strong | Self::Brand)
    }
}

/// Glyphs with 7-bit fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Bar,
    Rule,
    Heavy,
    Crumb,
    Observed,
    Inferred,
    Supplied,
    Assumed,
    Flag,
    Check,
    Cross,
    Half,
    Rail,
    RailEnd,
    Branch,
    Arrow,
    PlusMinus,
    Ellipsis,
    MeterFull,
    MeterEmpty,
    Dot,
    Prompt,
}

impl Glyph {
    fn text(self, unicode: bool) -> &'static str {
        if unicode {
            match self {
                Self::Bar => "▌",
                Self::Rule => "─",
                Self::Heavy => "━",
                Self::Crumb => "›",
                Self::Observed => "●",
                Self::Inferred => "○",
                Self::Supplied => "◇",
                Self::Assumed => "◆",
                Self::Flag => "▲",
                Self::Check => "✓",
                Self::Cross => "✕",
                Self::Half => "◐",
                Self::Rail => "│",
                Self::RailEnd => "╵",
                Self::Branch => "├─",
                Self::Arrow => "→",
                Self::PlusMinus => "±",
                Self::Ellipsis => "…",
                Self::MeterFull => "█",
                Self::MeterEmpty => "░",
                Self::Dot => "·",
                Self::Prompt => "›",
            }
        } else {
            match self {
                Self::Bar => "|",
                Self::Rule => "-",
                Self::Heavy => "=",
                Self::Crumb => ">",
                Self::Observed => "*",
                Self::Inferred => "o",
                Self::Supplied => "<>",
                Self::Assumed => "~",
                Self::Flag => "!",
                Self::Check => "+",
                Self::Cross => "x",
                Self::Half => "~",
                Self::Rail => "|",
                Self::RailEnd => "'",
                Self::Branch => "|-",
                Self::Arrow => "->",
                Self::PlusMinus => "+/-",
                Self::Ellipsis => "...",
                Self::MeterFull => "#",
                Self::MeterEmpty => ".",
                Self::Dot => ".",
                Self::Prompt => ">",
            }
        }
    }
}

/// A status badge: glyph plus an uppercase label in its tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    Ok,
    Warn,
    Flag,
    Info,
    Off,
}

impl Badge {
    fn parts(self) -> (Glyph, Tone) {
        match self {
            Self::Ok => (Glyph::Check, Tone::Ok),
            Self::Warn => (Glyph::Half, Tone::Warn),
            Self::Flag => (Glyph::Cross, Tone::Flag),
            Self::Info => (Glyph::Observed, Tone::Inferred),
            Self::Off => (Glyph::Inferred, Tone::Subtle),
        }
    }
}

/// Column alignment in [`Ui::table`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// One table column.
#[derive(Debug, Clone)]
pub struct Column {
    pub title: &'static str,
    pub align: Align,
    /// Longer cells are cut with an ellipsis.
    pub max: Option<usize>,
}

impl Column {
    pub fn left(title: &'static str) -> Self {
        Self {
            title,
            align: Align::Left,
            max: None,
        }
    }
    pub fn right(title: &'static str) -> Self {
        Self {
            title,
            align: Align::Right,
            max: None,
        }
    }
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }
}

/// One table cell: raw text plus the tone it is painted in. An optional prefix glyph is
/// painted in its own tone and counts towards the width.
#[derive(Debug, Clone)]
pub struct Cell {
    pub text: String,
    pub tone: Tone,
    pub mark: Option<(Glyph, Tone)>,
}

impl Cell {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            mark: None,
        }
    }
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Text)
    }
    pub fn marked(mut self, glyph: Glyph, tone: Tone) -> Self {
        self.mark = Some((glyph, tone));
        self
    }
}

/// Left margin of every content line.
pub const MARGIN: &str = "  ";
/// Width of the label column in key/value blocks.
pub const LABEL_WIDTH: usize = 18;
const DEFAULT_WIDTH: usize = 100;
const MIN_WIDTH: usize = 60;
const MAX_WIDTH: usize = 120;

/// The resolved rendering style of one invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ui {
    pub color: ColorLevel,
    pub stderr_color: ColorLevel,
    pub unicode: bool,
    /// Total line width used for rules and wrapping.
    pub width: usize,
}

impl Default for Ui {
    fn default() -> Self {
        Self::plain()
    }
}

impl Ui {
    /// No colour, Unicode glyphs, 100 columns: what pipes, files and tests get.
    pub fn plain() -> Self {
        Self {
            color: ColorLevel::None,
            stderr_color: ColorLevel::None,
            unicode: true,
            width: DEFAULT_WIDTH,
        }
    }

    /// Resolves the style from `--color`, `--ascii` and the terminal.
    pub fn resolve(choice: ColorChoice, ascii: bool, term: &TerminalEnv) -> Self {
        let level = |tty: bool| match choice {
            ColorChoice::Never => ColorLevel::None,
            ColorChoice::Always => term.depth(),
            ColorChoice::Auto if term.no_color || term.is_dumb() => ColorLevel::None,
            ColorChoice::Auto if tty || term.force_color => term.depth(),
            ColorChoice::Auto => ColorLevel::None,
        };
        let width = term
            .columns
            .filter(|_| term.stdout_tty)
            .unwrap_or(DEFAULT_WIDTH)
            .clamp(MIN_WIDTH, MAX_WIDTH);
        Self {
            color: level(term.stdout_tty),
            stderr_color: level(term.stderr_tty),
            unicode: !ascii && !term.is_dumb(),
            width,
        }
    }

    /// The same style, painted for stderr.
    pub fn for_stderr(&self) -> Self {
        Self {
            color: self.stderr_color,
            ..self.clone()
        }
    }

    pub fn colored(&self) -> bool {
        self.color != ColorLevel::None
    }

    /* --------------------------------------------------------------------- primitives */

    pub fn g(&self, glyph: Glyph) -> &'static str {
        glyph.text(self.unicode)
    }

    fn sgr(&self, tone: Tone, bold: bool, italic: bool) -> String {
        let mut codes: Vec<String> = Vec::new();
        if bold {
            codes.push("1".into());
        }
        if italic {
            codes.push("3".into());
        }
        if let Some((r, g, b)) = tone.rgb() {
            match self.color {
                ColorLevel::TrueColor => codes.push(format!("38;2;{r};{g};{b}")),
                ColorLevel::Ansi256 => codes.push(format!("38;5;{}", rgb_to_256(r, g, b))),
                ColorLevel::None => {}
            }
        }
        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }

    fn styled(&self, tone: Tone, text: &str, bold: bool, italic: bool) -> String {
        if !self.colored() || text.is_empty() {
            return text.to_string();
        }
        let open = self.sgr(tone, bold, italic);
        if open.is_empty() {
            text.to_string()
        } else {
            format!("{open}{text}\x1b[0m")
        }
    }

    /// `text` in `tone` (bold for `Strong` and `Brand`).
    pub fn paint(&self, tone: Tone, text: &str) -> String {
        self.styled(tone, text, tone.bold(), tone == Tone::Inferred)
    }

    /// `text` in `tone`, always bold.
    pub fn bold(&self, tone: Tone, text: &str) -> String {
        self.styled(tone, text, true, false)
    }

    /// A glyph painted in `tone`.
    pub fn mark(&self, glyph: Glyph, tone: Tone) -> String {
        self.paint(tone, self.g(glyph))
    }

    /// `● LABEL` style status badge.
    pub fn badge(&self, badge: Badge, label: &str) -> String {
        let (glyph, tone) = badge.parts();
        format!(
            "{} {}",
            self.mark(glyph, tone),
            self.bold(tone, &label.to_uppercase())
        )
    }

    fn rule_text(&self, glyph: Glyph, len: usize) -> String {
        self.g(glyph).repeat(len)
    }

    /* ------------------------------------------------------------------------ layout */

    /// Brand header: `▌ YUKTI │ CASE › SHOW            meta`, then a rule.
    pub fn header(&self, crumbs: &[&str], meta: &str) -> String {
        let bar = self.g(Glyph::Bar);
        let sep = format!(" {} ", self.g(Glyph::Crumb));
        let crumb_raw = crumbs
            .iter()
            .map(|c| c.to_uppercase())
            .collect::<Vec<_>>()
            .join(&sep);
        let left_len = 1 + bar.chars().count() + " YUKTI ".len() + 2 + crumb_raw.chars().count();
        let gap = self
            .width
            .saturating_sub(left_len + meta.chars().count() + 1)
            .max(2);
        let crumbs_painted = crumbs
            .iter()
            .map(|c| self.bold(Tone::Strong, &c.to_uppercase()))
            .collect::<Vec<_>>()
            .join(&self.paint(Tone::Subtle, &sep));
        let mut out = format!(
            " {} {} {} {}{}{}\n",
            self.paint(Tone::Brand, bar),
            self.paint(Tone::Brand, "YUKTI"),
            self.paint(Tone::Line, self.g(Glyph::Rail)),
            crumbs_painted,
            " ".repeat(gap),
            self.paint(Tone::Subtle, meta)
        );
        out.push(' ');
        out.push_str(&self.paint(
            Tone::Line,
            &self.rule_text(Glyph::Heavy, self.width.saturating_sub(2)),
        ));
        out.push('\n');
        out
    }

    /// Section heading `  TITLE  meta ──────────`, preceded by a blank line.
    pub fn section(&self, title: &str, meta: &str) -> String {
        let title_up = title.to_uppercase();
        let mut used = MARGIN.len() + title_up.chars().count() + 1;
        let mut out = format!("\n{MARGIN}{}", self.bold(Tone::Strong, &title_up));
        if !meta.is_empty() {
            out.push_str(&format!("  {}", self.paint(Tone::Subtle, meta)));
            used += 2 + meta.chars().count();
        }
        let rule = self.width.saturating_sub(used + 1).max(3);
        out.push(' ');
        out.push_str(&self.paint(Tone::Line, &self.rule_text(Glyph::Rule, rule)));
        out.push('\n');
        out
    }

    /// `  label             value` with a muted label; long values wrap under the value column.
    pub fn kv(&self, label: &str, tone: Tone, value: &str) -> String {
        let indent = MARGIN.len() + LABEL_WIDTH;
        let lines = wrap(value, self.width.saturating_sub(indent).max(20));
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                let _ = writeln!(
                    out,
                    "{MARGIN}{}{}",
                    self.paint(Tone::Muted, &label_cell(label)),
                    self.paint(tone, line)
                );
            } else {
                let _ = writeln!(out, "{}{}", " ".repeat(indent), self.paint(tone, line));
            }
        }
        out
    }

    /// Like [`Ui::kv`] for a value that is already painted (not wrapped).
    pub fn kv_painted(&self, label: &str, painted: &str) -> String {
        format!(
            "{MARGIN}{}{painted}\n",
            self.paint(Tone::Muted, &label_cell(label))
        )
    }

    /// Wrapped paragraph at `indent` columns in `tone`.
    pub fn para(&self, indent: usize, tone: Tone, text: &str) -> String {
        let width = self.width.saturating_sub(indent).max(20);
        let mut out = String::new();
        for line in text.lines() {
            for wrapped in wrap(line, width) {
                let _ = writeln!(out, "{}{}", " ".repeat(indent), self.paint(tone, &wrapped));
            }
        }
        out
    }

    /// Wrapped text after a painted prefix whose visible width is `prefix_width`; following
    /// lines start with `cont` (already painted, visible width `prefix_width`).
    pub fn hang(
        &self,
        prefix: &str,
        cont: &str,
        prefix_width: usize,
        tone: Tone,
        text: &str,
    ) -> String {
        let width = self.width.saturating_sub(prefix_width).max(20);
        let mut out = String::new();
        let mut first = true;
        for line in text.lines() {
            for wrapped in wrap(line, width) {
                let lead = if first { prefix } else { cont };
                first = false;
                let _ = writeln!(out, "{lead}{}", self.paint(tone, &wrapped));
            }
        }
        if first {
            let _ = writeln!(out, "{prefix}");
        }
        out
    }

    /// `  › text` next-step hint.
    pub fn hint(&self, text: &str) -> String {
        format!(
            "{MARGIN}{} {}\n",
            self.paint(Tone::Brand, self.g(Glyph::Prompt)),
            self.paint(Tone::Subtle, text)
        )
    }

    /// An OSC 8 terminal hyperlink around already-painted `text` (plain text without colour).
    pub fn link(&self, url: &str, text: &str) -> String {
        if self.colored() {
            format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
        } else {
            text.to_string()
        }
    }

    /// `  › label  command` next-step hint with the command highlighted.
    pub fn next(&self, label: &str, command: &str) -> String {
        format!(
            "{MARGIN}{} {}{}\n",
            self.paint(Tone::Brand, self.g(Glyph::Prompt)),
            self.paint(Tone::Subtle, &pad(label, 14)),
            self.paint(Tone::Text, command)
        )
    }

    /// The evidence-semantics legend.
    pub fn legend(&self) -> String {
        let item = |g: Glyph, t: Tone, label: &str| {
            format!("{} {}", self.mark(g, t), self.paint(Tone::Subtle, label))
        };
        format!(
            "{MARGIN}{}   {}   {}   {}   {}\n",
            item(Glyph::Observed, Tone::Text, "observed"),
            item(Glyph::Inferred, Tone::Inferred, "inferred"),
            item(Glyph::Supplied, Tone::Supplied, "supplied"),
            item(Glyph::Assumed, Tone::Warn, "assumed"),
            item(Glyph::Flag, Tone::Flag, "flagged for review"),
        )
    }

    /// `█████░░░░░` proportion meter (`value` is clamped to 0..=1).
    pub fn meter(&self, value: f64, width: usize, tone: Tone) -> String {
        let v = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let full = (v * width as f64).round() as usize;
        format!(
            "{}{}",
            self.paint(tone, &self.g(Glyph::MeterFull).repeat(full)),
            self.paint(
                Tone::Line,
                &self.g(Glyph::MeterEmpty).repeat(width.saturating_sub(full))
            )
        )
    }

    /// Big-number stat tiles in one row: `  12  events    3  flagged`.
    pub fn stats(&self, items: &[(String, &str, Tone)]) -> String {
        let mut out = String::from(MARGIN);
        for (i, (value, label, tone)) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(&self.paint(Tone::Line, &format!("  {}  ", self.g(Glyph::Rail))));
            }
            out.push_str(&self.bold(*tone, value));
            out.push(' ');
            out.push_str(&self.paint(Tone::Muted, label));
        }
        out.push('\n');
        out
    }

    /// Aligned table with an uppercase header and a rule under it.
    pub fn table(&self, columns: &[Column], rows: &[Vec<Cell>]) -> String {
        let n = columns.len();
        let cell_len = |c: &Cell, max: Option<usize>| {
            let mark = c.mark.map_or(0, |(g, _)| self.g(g).chars().count() + 1);
            let text = c.text.chars().count();
            mark + max.map_or(text, |m| text.min(m))
        };
        let mut widths: Vec<usize> = columns.iter().map(|c| c.title.chars().count()).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate().take(n) {
                widths[i] = widths[i].max(cell_len(cell, columns[i].max));
            }
        }
        let gap = "  ";
        let mut out = String::new();
        // Header.
        out.push_str(MARGIN);
        for (i, col) in columns.iter().enumerate() {
            let text = if i + 1 == n && col.align == Align::Left {
                col.title.to_string()
            } else {
                align(col.title, widths[i], col.align)
            };
            out.push_str(&self.paint(Tone::Subtle, &text));
            if i + 1 < n {
                out.push_str(gap);
            }
        }
        out.push('\n');
        let total: usize = widths.iter().sum::<usize>() + gap.len() * n.saturating_sub(1);
        out.push_str(MARGIN);
        out.push_str(&self.paint(
            Tone::Line,
            &self.rule_text(
                Glyph::Rule,
                total.min(self.width.saturating_sub(MARGIN.len())),
            ),
        ));
        out.push('\n');
        // Rows.
        for row in rows {
            out.push_str(MARGIN);
            for (i, col) in columns.iter().enumerate() {
                let empty = Cell::text("");
                let cell = row.get(i).unwrap_or(&empty);
                let text = match col.max {
                    Some(max) => truncate(&cell.text, max, self.g(Glyph::Ellipsis)),
                    None => cell.text.clone(),
                };
                let mark_len = cell.mark.map_or(0, |(g, _)| self.g(g).chars().count() + 1);
                let width = widths[i].saturating_sub(mark_len);
                let body = if i + 1 == n && col.align == Align::Left {
                    text
                } else {
                    align(&text, width, col.align)
                };
                if let Some((g, t)) = cell.mark {
                    out.push_str(&self.mark(g, t));
                    out.push(' ');
                }
                out.push_str(&self.paint(cell.tone, &body));
                if i + 1 < n {
                    out.push_str(gap);
                }
            }
            out.push('\n');
        }
        out
    }

    /// Closing line: `  3 case(s) · in the order returned by the API`.
    pub fn footnote(&self, text: &str) -> String {
        format!("\n{}", self.para(MARGIN.len(), Tone::Subtle, text))
    }

    /// Paints cargo-style `error:` / `warning:` / `note:` prefixes in a stderr text block.
    pub fn style_diagnostics(&self, text: &str) -> String {
        if !self.colored() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + 64);
        for line in text.split_inclusive('\n') {
            let styled = [
                ("error:", Tone::Flag),
                ("warning:", Tone::Warn),
                ("note:", Tone::Supplied),
            ]
            .iter()
            .find_map(|(prefix, tone)| {
                line.strip_prefix(prefix)
                    .map(|rest| format!("{}{rest}", self.bold(*tone, prefix)))
            });
            out.push_str(&styled.unwrap_or_else(|| line.to_string()));
        }
        out
    }
}

/* ------------------------------------------------------------------------------ helpers */

/// The YUKTI wordmark (ANSI Shadow), six rows.
pub const WORDMARK: [&str; 6] = [
    "██╗   ██╗██╗   ██╗██╗  ██╗████████╗██╗",
    "╚██╗ ██╔╝██║   ██║██║ ██╔╝╚══██╔══╝██║",
    " ╚████╔╝ ██║   ██║█████╔╝    ██║   ██║",
    "  ╚██╔╝  ██║   ██║██╔═██╗    ██║   ██║",
    "   ██║   ╚██████╔╝██║  ██╗   ██║   ██║",
    "   ╚═╝    ╚═════╝ ╚═╝  ╚═╝   ╚═╝   ╚═╝",
];

/// Tagline shown under the wordmark.
pub const TAGLINE: &str = "Digital Evidence Correlation & Crime Reconstruction";

impl Ui {
    /// The wordmark in crimson with its tagline (ASCII mode prints a one-line mark instead).
    pub fn banner(&self) -> String {
        let mut out = String::from("\n");
        if self.unicode {
            for line in WORDMARK {
                let _ = writeln!(out, "{MARGIN}{}", self.paint(Tone::Brand, line));
            }
        } else {
            let _ = writeln!(out, "{MARGIN}{}", self.paint(Tone::Brand, "[ Y U K T I ]"));
        }
        let _ = writeln!(
            out,
            "{MARGIN}{}  {}",
            self.paint(Tone::Muted, TAGLINE),
            self.paint(
                Tone::Subtle,
                &format!("{} cli v{}", self.g(Glyph::Dot), env!("CARGO_PKG_VERSION"))
            )
        );
        out
    }
}

/// A key/value label padded to the label column, always followed by at least two spaces.
fn label_cell(label: &str) -> String {
    pad(label, LABEL_WIDTH.max(label.chars().count() + 2))
}

/// `text` padded with spaces to `width` characters.
pub fn pad(text: &str, width: usize) -> String {
    align(text, width, Align::Left)
}

fn align(text: &str, width: usize, how: Align) -> String {
    let len = text.chars().count();
    let fill = " ".repeat(width.saturating_sub(len));
    match how {
        Align::Left => format!("{text}{fill}"),
        Align::Right => format!("{fill}{text}"),
    }
}

/// Cuts `text` to `max` characters, ending in `ellipsis` when cut.
pub fn truncate(text: &str, max: usize, ellipsis: &str) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let keep = max.saturating_sub(ellipsis.chars().count());
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(ellipsis);
    out
}

/// Greedy word wrap to `width` characters. Words longer than the width (hashes, URLs) are
/// kept whole on their own line rather than split.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let needed = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if needed > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// First 8 characters of a UUID followed by an ellipsis; other identifiers unchanged.
pub fn short_id(id: &str, ui: &Ui) -> String {
    if crate::dto::is_hyphenated_uuid(id) {
        format!("{}{}", &id[..8], ui.g(Glyph::Ellipsis))
    } else {
        id.to_string()
    }
}

/// Removes ANSI SGR sequences (for width checks and tests).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Nearest xterm-256 colour for an sRGB value.
pub fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    // Greys map to the 24-step ramp when the channels are close.
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max - min < 10 {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return (((u16::from(r) - 8) * 24 / 247) as u8) + 232;
    }
    let level = |v: u8| -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            ((u16::from(v) - 35) / 40) as u8
        }
    };
    16 + 36 * level(r) + 6 * level(g) + level(b)
}

/// Human byte size: `1.4 MiB (1468006 bytes)`.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {} ({bytes} bytes)", UNITS[unit])
}

/// Badge for an UPPER_SNAKE status coming from the API.
pub fn status_badge(status: &str) -> Badge {
    let s = status.to_ascii_uppercase();
    if [
        "FAILED",
        "CORRUPTED",
        "EXTRACTION_FAILED",
        "BLOCKED",
        "HASH_MISMATCH",
        "MISSING_FROM_STORAGE",
        "CRITICAL",
        "HIGH",
        "SHIP_EXPORT_BLOCKED",
        "NOT_READY",
    ]
    .contains(&s.as_str())
    {
        Badge::Flag
    } else if [
        "PARTIAL_SUCCESS",
        "INTERRUPTED",
        "PENDING",
        "RUNNING",
        "EXTRACTION_PENDING",
        "MEDIUM",
        "DEGRADED",
        "UNKNOWN_EVIDENCE",
    ]
    .contains(&s.as_str())
    {
        Badge::Warn
    } else if [
        "SUCCESS",
        "EXTRACTED",
        "CLEAR",
        "VERIFIED_MATCH",
        "SHIP_EXPORT_CLEARED",
        "READY",
        "CONNECTED",
        "ACTIVE",
        "OPEN",
        "INGESTED",
    ]
    .contains(&s.as_str())
    {
        Badge::Ok
    } else if s == "LOW" {
        Badge::Info
    } else {
        Badge::Off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tty() -> TerminalEnv {
        TerminalEnv {
            stdout_tty: true,
            stderr_tty: true,
            colorterm: Some("truecolor".into()),
            columns: Some(90),
            ..TerminalEnv::default()
        }
    }

    #[test]
    fn colour_follows_terminal_and_no_color() {
        let ui = Ui::resolve(ColorChoice::Auto, false, &tty());
        assert_eq!(ui.color, ColorLevel::TrueColor);
        assert_eq!(ui.width, 90);

        let piped = Ui::resolve(ColorChoice::Auto, false, &TerminalEnv::default());
        assert_eq!(piped.color, ColorLevel::None);
        assert_eq!(piped.width, 100);

        let no_color = TerminalEnv {
            no_color: true,
            ..tty()
        };
        assert_eq!(
            Ui::resolve(ColorChoice::Auto, false, &no_color).color,
            ColorLevel::None
        );
        assert_eq!(
            Ui::resolve(ColorChoice::Always, false, &TerminalEnv::default()).color,
            ColorLevel::Ansi256
        );
        assert_eq!(
            Ui::resolve(ColorChoice::Never, false, &tty()).color,
            ColorLevel::None
        );
        let dumb = TerminalEnv {
            term: Some("dumb".into()),
            ..tty()
        };
        let ui = Ui::resolve(ColorChoice::Auto, false, &dumb);
        assert_eq!(ui.color, ColorLevel::None);
        assert!(!ui.unicode);
    }

    #[test]
    fn plain_output_has_no_escape_codes_and_colour_output_strips_to_it() {
        let plain = Ui::plain();
        let rich = Ui {
            color: ColorLevel::TrueColor,
            ..Ui::plain()
        };
        let render = |ui: &Ui| {
            let mut s = ui.header(&["case", "show"], "http://127.0.0.1:8080");
            s.push_str(&ui.section("Timeline", "3 events"));
            s.push_str(&ui.kv("Case ID", Tone::Text, "abc"));
            s.push_str(&ui.badge(Badge::Flag, "blocked"));
            s.push_str(&ui.meter(0.5, 10, Tone::Ok));
            s.push_str(&ui.table(
                &[Column::left("A"), Column::right("N")],
                &[vec![
                    Cell::text("x").marked(Glyph::Flag, Tone::Flag),
                    Cell::text("12"),
                ]],
            ));
            s
        };
        let p = render(&plain);
        let r = render(&rich);
        assert!(!p.contains('\x1b'));
        assert!(r.contains("\x1b[38;2;229;36;59m"));
        assert_eq!(strip_ansi(&r), p);
    }

    #[test]
    fn ascii_mode_uses_seven_bit_glyphs_only() {
        let ui = Ui {
            unicode: false,
            ..Ui::plain()
        };
        let s = format!(
            "{}{}{}{}",
            ui.header(&["status"], "meta"),
            ui.section("x", ""),
            ui.legend(),
            ui.meter(0.3, 8, Tone::Ok)
        );
        assert!(s.is_ascii(), "{s}");
    }

    #[test]
    fn header_and_sections_fit_the_width() {
        let ui = Ui {
            width: 72,
            ..Ui::plain()
        };
        for line in ui
            .header(&["reconstruction", "explain"], "http://h:1")
            .lines()
            .chain(ui.section("contradiction leads", "2 leads").lines())
        {
            assert!(line.chars().count() <= 72, "{line:?}");
        }
    }

    #[test]
    fn wrap_keeps_long_words_whole() {
        assert_eq!(wrap("a bb ccc", 4), vec!["a bb", "ccc"]);
        let hash = "f".repeat(64);
        assert_eq!(
            wrap(&format!("sha {hash}"), 20),
            vec!["sha".to_string(), hash]
        );
        assert_eq!(wrap("", 10), vec![String::new()]);
    }

    #[test]
    fn table_aligns_columns() {
        let ui = Ui::plain();
        let t = ui.table(
            &[
                Column::left("NAME"),
                Column::right("COUNT"),
                Column::left("NOTE"),
            ],
            &[
                vec![Cell::text("alpha"), Cell::text("3"), Cell::text("x")],
                vec![
                    Cell::text("b"),
                    Cell::text("120"),
                    Cell::text("longer note"),
                ],
            ],
        );
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines[0], "  NAME   COUNT  NOTE");
        assert_eq!(lines[2], "  alpha      3  x");
        assert_eq!(lines[3], "  b        120  longer note");
    }

    #[test]
    fn diagnostics_are_painted_only_in_colour_mode() {
        let text = "error: boom\nnote: hint\nplain\n";
        assert_eq!(Ui::plain().style_diagnostics(text), text);
        let rich = Ui {
            color: ColorLevel::Ansi256,
            ..Ui::plain()
        };
        let styled = rich.style_diagnostics(text);
        assert_ne!(styled, text);
        assert_eq!(strip_ansi(&styled), text);
    }

    #[test]
    fn palette_maps_to_256_colours() {
        assert_eq!(rgb_to_256(0, 0, 0), 16);
        assert_eq!(rgb_to_256(255, 255, 255), 231);
        assert_eq!(rgb_to_256(0xE5, 0x24, 0x3B), 161);
        assert!((232..=255).contains(&rgb_to_256(0x71, 0x71, 0x7A)));
    }

    #[test]
    fn badges_classify_api_statuses() {
        assert_eq!(status_badge("SUCCESS"), Badge::Ok);
        assert_eq!(status_badge("partial_success"), Badge::Warn);
        assert_eq!(status_badge("BLOCKED"), Badge::Flag);
        assert_eq!(status_badge("SOMETHING_NEW"), Badge::Off);
    }
}
