//! The wire-format registry: which declared shape an engine's requests are
//! built with, and which of those shapes can accept image input.
//!
//! The format is **declared**, never inferred (D-24): an engine names it
//! through `IMAGAI__ENGINES__<NAME>__FORMAT`, and a declaration always wins
//! over any inference (D-29). The existing `src/provider.rs` string sniffing
//! stays exactly where it is, as the default for engines that declare nothing
//! (D-28) — this module decides *which family* an engine speaks, and
//! `src/provider.rs` keeps deciding *which transport inside the
//! OpenAI-compatible family*.

use crate::config::EngineConfig;

/// The wire format a provider adapter speaks. The closed set of declared names
/// (D-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFormat {
    /// The OpenAI-compatible family, including the DALL-E, Stability and
    /// OpenRouter-vision engines the crate already speaks.
    OpenAiCompatible,
    /// The one named non-OpenAI-compatible adapter shipped in v1: fal.ai
    /// (D-25). Generation-only (D-26).
    Fal,
}

impl ProviderFormat {
    /// The declared names this build recognises, in the order they are reported
    /// to the user.
    pub const FORMAT_NAMES: [&str; 2] = ["openai", "fal"];

    /// The declared name for this format.
    ///
    /// This is the string every surface displays and the string `.env.example`
    /// documents (D-38).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "openai",
            Self::Fal => "fal",
        }
    }

    /// Parse a declared `IMAGAI__ENGINES__<NAME>__FORMAT` value.
    ///
    /// Case-insensitive and whitespace-tolerant; anything outside the closed set
    /// is `None` (D-24).
    pub fn from_declared(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Some(Self::OpenAiCompatible),
            "fal" => Some(Self::Fal),
            _ => None,
        }
    }

    /// Whether this format family can accept image input at all.
    ///
    /// The OpenAI-compatible family can (the transport inside it is chosen per
    /// model); the generation-only format cannot (D-26, D-30).
    ///
    /// No per-model rule lives here: whether a *particular* OpenAI-compatible
    /// engine is edit-capable stays `edit_transport`'s existing model matching,
    /// which D-28 keeps. D-30 changes only which family may accept image input,
    /// not how the OpenAI-compatible family picks its transport.
    pub fn supports_image_input(&self) -> bool {
        matches!(self, Self::OpenAiCompatible)
    }

    /// Resolve the format an engine's requests will be built with.
    ///
    /// Precedence is D-29's: a trimmed, non-empty declared value must be a
    /// member of the closed set, or this returns an error naming the engine.
    /// With nothing declared the OpenAI-compatible family is the default and the
    /// existing sniffing applies (D-28) — the within-family sniffing
    /// (`model.contains("dall-e-3")`, `stability`, `openrouter.ai` + `gemini`)
    /// stays in `src/provider.rs`, untouched by this phase.
    pub fn resolve(engine: &str, cfg: &EngineConfig) -> anyhow::Result<Self> {
        if let Some(declared) = cfg
            .format
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            return match Self::from_declared(declared) {
                Some(f) => Ok(f),
                None => anyhow::bail!(
                    "Engine '{engine}' declares an unknown FORMAT '{declared}'; supported formats: {}",
                    Self::FORMAT_NAMES.join(", ")
                ),
            };
        }
        Ok(Self::OpenAiCompatible)
    }

    /// The format name a surface displays.
    ///
    /// The resolved name when the engine resolves; the declared value verbatim
    /// when it does not, so a typo is visible in `list-engines` rather than
    /// hidden behind a resolved default. This function is the one display helper
    /// the CLI, TUI and web surfaces all call, so the column cannot drift
    /// between them.
    pub fn format_label(cfg: &EngineConfig) -> String {
        match cfg
            .format
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            Some(declared) => match Self::from_declared(declared) {
                Some(f) => f.as_str().to_string(),
                None => declared.to_string(),
            },
            None => Self::OpenAiCompatible.as_str().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EngineConfig;

    fn cfg(format: Option<&str>) -> EngineConfig {
        EngineConfig {
            api_key: "sk".to_string(),
            base_url: Some("http://mock/v1".to_string()),
            model: Some("dall-e-3".to_string()),
            format: format.map(str::to_string),
        }
    }

    #[test]
    fn declared_names_are_case_and_whitespace_tolerant() {
        assert_eq!(
            ProviderFormat::from_declared("openai"),
            Some(ProviderFormat::OpenAiCompatible)
        );
        assert_eq!(
            ProviderFormat::from_declared("  FAL  "),
            Some(ProviderFormat::Fal)
        );
        assert_eq!(
            ProviderFormat::from_declared("OpenAI"),
            Some(ProviderFormat::OpenAiCompatible)
        );
    }

    #[test]
    fn declared_names_reject_everything_outside_the_closed_set() {
        for bad in [
            "",
            "  ",
            "falai",
            "open ai",
            "openai-compatible",
            "stability",
        ] {
            assert_eq!(
                ProviderFormat::from_declared(bad),
                None,
                "'{bad}' must be rejected"
            );
        }
    }

    #[test]
    fn only_the_openai_compatible_family_takes_image_input() {
        assert!(ProviderFormat::OpenAiCompatible.supports_image_input());
        assert!(!ProviderFormat::Fal.supports_image_input());
    }

    #[test]
    fn undeclared_engines_resolve_to_the_openai_compatible_family() {
        assert_eq!(
            ProviderFormat::resolve("mock", &cfg(None)).unwrap(),
            ProviderFormat::OpenAiCompatible
        );
    }

    #[test]
    fn a_declaration_beats_every_inference() {
        // An OpenAI base_url and a `dall-e-3` model would both sniff the
        // OpenAI-compatible family; the declaration still wins (D-29).
        let resolved =
            ProviderFormat::resolve("falengine", &cfg(Some("fal"))).expect("fal resolves");
        assert_eq!(resolved, ProviderFormat::Fal);
    }

    #[test]
    fn a_declared_openai_format_keeps_the_within_family_sniffing() {
        let declared = EngineConfig {
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            model: Some("google/gemini-2.0-flash-image".to_string()),
            format: Some("openai".to_string()),
            ..cfg(None)
        };
        assert_eq!(
            ProviderFormat::resolve("gem", &declared).unwrap(),
            ProviderFormat::OpenAiCompatible
        );
    }

    #[test]
    fn an_unknown_declaration_names_the_engine_and_the_supported_names() {
        let err = ProviderFormat::resolve("typo_engine", &cfg(Some("bogus")))
            .unwrap_err()
            .to_string();
        assert!(err.contains("typo_engine"), "{err}");
        assert!(err.contains("bogus"), "{err}");
        assert!(err.contains("openai"), "{err}");
        assert!(err.contains("fal"), "{err}");
    }

    #[test]
    fn a_blank_declaration_is_treated_as_no_declaration() {
        assert_eq!(
            ProviderFormat::resolve("mock", &cfg(Some("   "))).unwrap(),
            ProviderFormat::OpenAiCompatible
        );
    }

    #[test]
    fn format_label_shows_the_resolved_name_or_the_raw_typo() {
        assert_eq!(ProviderFormat::format_label(&cfg(None)), "openai");
        assert_eq!(ProviderFormat::format_label(&cfg(Some("fal"))), "fal");
        assert_eq!(ProviderFormat::format_label(&cfg(Some("bogus"))), "bogus");
        assert_eq!(ProviderFormat::format_label(&cfg(Some(" FAL "))), "fal");
    }
}
