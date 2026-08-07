//! The `catalog` command — print the block vocabulary the agent authors against.
//!
//! A thin shell over [`ags_render::block_catalog`]: the catalog is generated from
//! the validator's own type set and attribute schema, so it can never drift from
//! what Gate 1 accepts. The authoring skill has the agent read it before authoring.

use std::process::ExitCode;

/// Print the block catalog to stdout and exit zero.
#[must_use]
pub fn run() -> ExitCode {
    print!("{}", ags_render::block_catalog());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    /// Freshness guard for the shipped authoring skill (artifact-authoring §6.2): it
    /// must point the agent at the live `ags catalog` rather than duplicate the
    /// vocabulary, so it cannot drift from the validator. If the skill goes missing,
    /// stops referencing the catalog, or inlines the per-type attribute schema, this
    /// fails — forcing the skill back in sync with the tool.
    #[test]
    fn authoring_skill_defers_to_the_live_catalog() {
        let skill = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skill/SKILL.md"))
            .expect("authoring skill present at crates/ags/skill/SKILL.md");
        assert!(
            skill.contains("ags catalog"),
            "skill must send the agent to the live catalog command"
        );
        // Every attribute enum belongs to the catalog alone — inlining any of them
        // would drift. Covers each `AttrValues::OneOf` set the validator defines.
        for inlined in [
            "radio|checkbox|text|select",
            "TD|LR|BT|RL",
            "none|annotate|comment",
            "static|live",
            "info|warn|claim",
        ] {
            assert!(
                !skill.contains(inlined),
                "skill inlines catalog schema ('{inlined}') — reference `ags catalog` instead"
            );
        }
    }
}
