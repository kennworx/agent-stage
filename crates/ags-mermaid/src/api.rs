//! The public surface: diagram text in, SVG text out.
//!
//! Two entry points. [`render_svg`] is what a consumer calls; [`inspect`] hands
//! back the scene instead of the string, so the constraint stage and the tests
//! never have to read geometry back out of emitted markup.
//!
//! Nothing here reads a filesystem, consults a clock, or starts a thread, so the
//! same code serves a server, a browser through WebAssembly, and a command line
//! producing a standalone image.

use crate::detect::{detect, Detection, DiagramType};
use crate::emit::svg;
use crate::scene::Scene;
use crate::theme::Theme;

/// Where the colours in the output come from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Reference theme tokens and let CSS derive the rest.
    ///
    /// For a diagram inside a page. Values follow any token change, so a page
    /// restyles every diagram by changing one variable, with no re-render.
    #[default]
    Tokens,
    /// Write literal colours throughout.
    ///
    /// For an image with no document behind it: a token reference in a
    /// standalone SVG resolves to nothing, and a raster has no cascade at all.
    Fixed,
}

/// How text is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Measure {
    /// The built-in character-width model.
    ///
    /// The default everywhere, including in a browser that could measure
    /// exactly: geometry derives from measurement, so a host-specific measurer
    /// would make the same source render differently in different places and
    /// leave every verification diff comparing nothing.
    #[default]
    Estimate,
}

/// Rendering options.
///
/// `PartialEq` but not `Eq`: the measurements a diagram is built from are
/// floating point, so there is no total equality to derive.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Options {
    pub colors: ColorMode,
    pub measure: Measure,
    /// The tokens a drawing derives its palette from.
    ///
    /// Only read in `Fixed` mode, where there is no page to supply them; in
    /// `Tokens` mode the page's own values win and these are the fallbacks
    /// behind them.
    pub theme: Theme,
    /// The measurements a flowchart is built from.
    ///
    /// Carried here rather than baked into the renderer so a caller can move one
    /// without editing it — a drawing set into a dense page and the same drawing
    /// exported on its own want different type sizes. `Default` is the drawing
    /// this crate has always made, so a caller that says nothing sees no change.
    pub flowchart: crate::flowchart::Config,
}

/// Why a diagram could not be rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The source declares nothing recognisable.
    ///
    /// `suggestion` is set when the header is one edit from a real keyword. The
    /// renderer this replaces treated anything unrecognised as a flowchart, so a
    /// typo drew the wrong diagram instead of saying so.
    UnknownType {
        found: String,
        suggestion: Option<&'static str>,
    },
    /// The source is the right kind but does not parse.
    Malformed { line: usize, message: String },
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownType { found, suggestion } => {
                if found.is_empty() {
                    return write!(f, "no diagram type declared");
                }
                match suggestion {
                    Some(s) => write!(f, "unknown diagram type `{found}` — did you mean `{s}`?"),
                    None => write!(f, "unknown diagram type `{found}`"),
                }
            }
            Self::Malformed { line, message } => write!(f, "line {line}: {message}"),
        }
    }
}

/// A rendered diagram, with whatever the drawing got wrong.
///
/// Violations ride alongside the SVG rather than replacing it. The two callers
/// want opposite things: an artifact gate refuses to ship a diagram with any,
/// while an editor rendering as someone types needs the drawing regardless and
/// would otherwise blank on every keystroke.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub svg: String,
    pub violations: Vec<crate::constraint::Violation>,
}

/// Lay a diagram out, without drawing it.
///
/// # Errors
/// When the source declares no recognisable diagram, or declares one this build
/// cannot draw.
pub fn inspect(source: &str, options: &Options) -> Result<Scene, RenderError> {
    let kind = match detect(source) {
        Detection::Known(kind) => kind,
        Detection::Unknown { found, suggestion } => {
            return Err(RenderError::UnknownType { found, suggestion })
        }
    };
    Ok(draw(kind, source, options))
}

/// Hand a source to the renderer for its kind.
///
/// Dispatched by `match` rather than through a trait object, so a build that
/// never mentions a type can have the linker drop it — which is what keeps a
/// WebAssembly page from carrying twenty-nine diagram types it never draws.
///
/// Total: every kind the detector knows how to name, this draws. There is no
/// "recognised but undrawn" arm left, and so no error for one.
fn draw(kind: DiagramType, source: &str, options: &Options) -> Scene {
    let (theme, mode) = (&options.theme, &options.colors);
    match kind {
        DiagramType::Block => crate::block::render(source, theme, mode),
        DiagramType::C4 => {
            crate::c4::scene(&crate::c4::layout(&crate::c4::parse(source)), theme, mode)
        }
        DiagramType::Packet => crate::packet::render(source, theme, mode),
        DiagramType::Pie => crate::pie::render(source, theme, mode),
        DiagramType::Quadrant => crate::quadrant::render(source, theme, mode),
        DiagramType::Sankey => crate::sankey::render(source, theme, mode),
        DiagramType::Timeline => crate::timeline::render(source, theme, mode),
        DiagramType::Wardley => crate::wardley::render(source, theme, mode),
        DiagramType::Journey => crate::journey::render(source, theme, mode),
        DiagramType::Venn => crate::venn::render(source, theme, mode),
        DiagramType::TreeView => crate::treeview::render(source, theme, mode),
        DiagramType::Ishikawa => crate::ishikawa::render(source, theme, mode),
        DiagramType::Radar => crate::radar::render(source, theme, mode),
        DiagramType::Kanban => crate::kanban::render(source, theme, mode),
        DiagramType::GitGraph => crate::gitgraph::render(source, theme, mode),
        DiagramType::Treemap => crate::treemap::render(source, theme, mode),
        DiagramType::EventModeling => crate::eventmodeling::render(source, theme, mode),
        DiagramType::Mindmap => crate::mindmap::render(source, theme, mode),
        DiagramType::Requirement => crate::requirement::render(source, theme, mode),
        DiagramType::Gantt => crate::gantt::render(source, theme, mode),
        DiagramType::ZenUml => crate::zenuml::render(source, theme, mode),
        DiagramType::Sequence => crate::sequence::render(source, theme, mode),
        DiagramType::XyChart => crate::xychart::render(source, &options.theme, &options.colors),
        DiagramType::Flowchart => crate::flowchart::render(source, theme, mode, &options.flowchart),
        DiagramType::Class => crate::class::render(source, theme, mode),
        DiagramType::Er => crate::er::render(source, theme, mode),
        DiagramType::Architecture => crate::architecture::render(source, theme, mode),
    }
}

/// Render a diagram to SVG.
///
/// # Errors
/// When the source declares no recognisable diagram, or declares one this build
/// cannot draw.
pub fn render_svg(source: &str, options: &Options) -> Result<Rendered, RenderError> {
    let scene = inspect(source, options)?;
    let violations = crate::constraint::check(&scene);
    // No pass over the finished document: the scene carries the colour config it
    // was built with, so the emitter already wrote every colour the way this
    // target needs it.
    Ok(Rendered {
        svg: svg(&scene),
        violations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_type_is_reported_with_its_header() {
        let err = render_svg("sunburstChart", &Options::default()).unwrap_err();
        assert_eq!(
            err,
            RenderError::UnknownType {
                found: "sunburstchart".into(),
                suggestion: None
            }
        );
    }

    #[test]
    fn a_typo_is_reported_as_one() {
        let err = inspect("pae title X", &Options::default()).unwrap_err();
        assert_eq!(
            err,
            RenderError::UnknownType {
                found: "pae".into(),
                suggestion: Some("pie")
            }
        );
        assert!(err.to_string().contains("did you mean `pie`"), "{err}");
    }

    #[test]
    fn every_type_the_detector_knows_can_be_drawn() {
        // There is no "known but undrawn" error left to test: the dispatch is
        // total, which is what the port set out to make true.
        for source in [
            "architecture-beta\n  group a(cloud)[Cloud]",
            "classDiagram\n  class A",
            "erDiagram\n  A ||--|| B : has",
            "graph TD\n  A --> B",
        ] {
            assert!(inspect(source, &Options::default()).is_ok(), "{source}");
        }
    }

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        let err = render_svg("", &Options::default()).unwrap_err();
        assert_eq!(err.to_string(), "no diagram type declared");
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            RenderError::Malformed {
                line: 4,
                message: "expected a value".into()
            }
            .to_string(),
            "line 4: expected a value"
        );
        assert_eq!(
            RenderError::UnknownType {
                found: "wat".into(),
                suggestion: None
            }
            .to_string(),
            "unknown diagram type `wat`"
        );
    }

    #[test]
    fn a_c4_diagram_is_drawn_rather_than_refused() {
        let out = render_svg(
            "C4Context\ntitle Hello\nPerson(a,\"A\")\nSystem(b,\"B\")\nRel(a,b,\"uses\")",
            &Options::default(),
        )
        .expect("a C4 diagram renders");
        assert!(out.svg.starts_with("<svg"), "{}", out.svg);
        assert!(out.svg.contains("data-id=\"a\""), "{}", out.svg);
        assert!(
            out.svg.contains("data-from=\"a\" data-to=\"b\""),
            "{}",
            out.svg
        );
        assert!(out.svg.ends_with("</svg>"));
    }

    #[test]
    fn every_c4_flavour_reaches_the_same_renderer() {
        for header in [
            "C4Context",
            "C4Container",
            "C4Component",
            "C4Dynamic",
            "C4Deployment",
            "journey\nA: 3: Me",
            "venn-beta\nset A\nset B",
            "treeView-beta\nroot/\n    child",
            "ishikawa\nEffect\n  Category",
            "radar-beta\naxis a, b\ncurve x{1, 2}",
            "kanban\ntodo[To do]\n    t1[One]",
            "gitGraph\ncommit\nbranch f\ncommit",
            "treemap\n\"a\" : 1\n\"b\" : 2",
            "eventmodeling\ntf 1 ui A\ntf 2 evt B",
            "mindmap\nroot\n  a\n  b",
            "requirementDiagram\nrequirement r {\nid: 1\n}",
            "gantt\nsection S\nA :2024-01-01, 5d",
            "zenuml\n    Alice->Bob: Request\n    Bob.process()\n    Bob->Alice: Response",
            "xychart-beta\n    title \"Sales\"\n    x-axis [A, B, C]\n    bar [10, 20, 30]",
        ] {
            let source = format!("{header}\nSystem(a,\"A\")");
            assert!(
                inspect(&source, &Options::default()).is_ok(),
                "{header} was refused"
            );
        }
    }

    #[test]
    fn every_type_this_build_draws_reaches_its_renderer() {
        // One source per native type. The list grows with the port, and a type
        // wired into the dispatch but never exercised here would look drawn
        // while nothing had ever drawn it.
        for source in [
            "C4Context\nPerson(a,\"A\")",
            "pie title X\n\"a\" : 1",
            "packet\n0-7: \"a\"",
            "timeline\n2024 : shipped",
            "block-beta\ncolumns 2\nA[\"Alpha\"] B[\"Beta\"]",
            "quadrantChart\nA: [0.3, 0.6]",
            "sankey-beta\nA,B,10",
            "wardley-beta\nA [0.5, 0.5]",
        ] {
            let scene = inspect(source, &Options::default())
                .unwrap_or_else(|err| panic!("{source} was refused: {err}"));
            assert!(scene.canvas.width > 0.0, "{source} drew nothing");
        }
    }

    #[test]
    fn fixed_colours_leave_no_token_for_a_missing_page_to_resolve() {
        let source = "C4Context\nPerson(a,\"A\")\nSystem(b,\"B\")\nRel(a,b,\"uses\")";
        let out = render_svg(
            source,
            &Options {
                colors: ColorMode::Fixed,
                ..Options::default()
            },
        )
        .expect("renders");
        // The public tokens are written out, so every `var(--ags-bg)` in the rules
        // has something to read.
        assert!(out.svg.contains("--ags-bg:#ffffff"), "{}", out.svg);
        assert!(!out.svg.contains("color-mix"), "{}", out.svg);
    }

    #[test]
    fn token_colours_are_the_default() {
        assert_eq!(Options::default().colors, ColorMode::Tokens);
        assert_eq!(Options::default().measure, Measure::Estimate);
    }
}
