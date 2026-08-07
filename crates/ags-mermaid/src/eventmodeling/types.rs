//! The parsed shape of an event model: time frames in three swimlanes.

/// What a frame is. The five kinds sort into three lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Ui,
    Processor,
    Command,
    ReadModel,
    Event,
}

/// The three bands a model is read across.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    UiAutomation,
    CommandReadModel,
    Events,
}

impl Entity {
    /// Which band this kind lives in.
    pub const fn lane(self) -> Lane {
        match self {
            Self::Ui | Self::Processor => Lane::UiAutomation,
            Self::Command | Self::ReadModel => Lane::CommandReadModel,
            Self::Event => Lane::Events,
        }
    }

    /// Its own palette slot, so the five kinds are told apart by colour.
    /// Every kind, so a reader of one of the tables below can be sure it is
    /// looking at all of them — and a test can be exhaustive by construction
    /// rather than by someone remembering to add a line.
    pub const ALL: [Self; 5] = [
        Self::Ui,
        Self::Processor,
        Self::Command,
        Self::ReadModel,
        Self::Event,
    ];

    pub const fn color_index(self) -> usize {
        match self {
            Self::Ui => 0,
            Self::Processor => 1,
            Self::Command => 2,
            Self::ReadModel => 3,
            Self::Event => 4,
        }
    }

    /// What is written under a frame's name.
    pub const fn caption(self) -> &'static str {
        match self {
            Self::Ui => "UI",
            Self::Processor => "Processor",
            Self::Command => "Command",
            Self::ReadModel => "Read Model",
            Self::Event => "Event",
        }
    }

    /// The token this kind is written as, which becomes its `data-type`.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Processor => "pcr",
            Self::Command => "cmd",
            Self::ReadModel => "rmo",
            Self::Event => "evt",
        }
    }
}

impl Lane {
    /// The bands, top to bottom.
    pub const ALL: [Self; 3] = [Self::UiAutomation, Self::CommandReadModel, Self::Events];

    pub const fn label(self) -> &'static str {
        match self {
            Self::UiAutomation => "UI / Automation",
            Self::CommandReadModel => "Command / Read Model",
            Self::Events => "Events",
        }
    }
}

/// A parsed event model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Model {
    pub title: Option<String>,
    pub frames: Vec<Frame>,
}

/// One time frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// As written, made unique — this is the drawn element's identity.
    pub number: String,
    /// The digits in that number, which is what decides column order.
    pub numeric: usize,
    pub entity: Entity,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_own_colour_caption_and_token() {
        // Three mapping tables over the same five kinds. Tested together and
        // exhaustively, because the failure they invite is one arm quietly
        // sharing another's answer, which no single-variant test can see.
        let mut colours: Vec<usize> = Entity::ALL.iter().map(|e| e.color_index()).collect();
        let mut captions: Vec<&str> = Entity::ALL.iter().map(|e| e.caption()).collect();
        let mut tokens: Vec<&str> = Entity::ALL.iter().map(|e| e.token()).collect();
        assert_eq!(colours.len(), 5);
        colours.sort_unstable();
        colours.dedup();
        captions.sort_unstable();
        captions.dedup();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(colours.len(), 5, "no two kinds share a series colour");
        assert_eq!(captions.len(), 5, "no two kinds share a caption");
        assert_eq!(tokens.len(), 5, "no two kinds share a token");
    }

    #[test]
    fn the_tokens_are_the_ones_an_author_writes() {
        // These reach the source and the `data-type` attribute, so they are a
        // contract rather than an implementation detail.
        assert_eq!(Entity::Ui.token(), "ui");
        assert_eq!(Entity::Processor.token(), "pcr");
        assert_eq!(Entity::Command.token(), "cmd");
        assert_eq!(Entity::ReadModel.token(), "rmo");
        assert_eq!(Entity::Event.token(), "evt");
    }

    #[test]
    fn a_two_word_kind_is_captioned_with_the_space() {
        assert_eq!(Entity::ReadModel.caption(), "Read Model");
    }

    #[test]
    fn every_lane_is_labelled() {
        let labels: Vec<&str> = Lane::ALL.iter().map(|l| l.label()).collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.iter().all(|l| !l.is_empty()));
    }
}
