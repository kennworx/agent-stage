//! The parsed shape of a gantt chart: sections of dated tasks.

/// A tag a task may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Done,
    Active,
    Crit,
    Milestone,
}

impl Status {
    /// The keyword this status is written as, which becomes its class.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Active => "active",
            Self::Crit => "crit",
            Self::Milestone => "milestone",
        }
    }

    /// The status a keyword names, if it names one.
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word.to_ascii_lowercase().as_str() {
            "done" => Some(Self::Done),
            "active" => Some(Self::Active),
            "crit" => Some(Self::Crit),
            "milestone" => Some(Self::Milestone),
            _ => None,
        }
    }
}

/// A parsed gantt chart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Chart {
    pub title: Option<String>,
    pub date_format: Option<String>,
    /// The earliest date any task names, when any of them named one. Without it
    /// the axis counts days from the start rather than showing dates.
    pub start_date: Option<String>,
    pub sections: Vec<Section>,
    /// Every task, in source order, across all sections.
    pub tasks: Vec<Task>,
}

/// One group of tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub tasks: Vec<Task>,
}

/// One task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Unique within the chart: the id the author gave, or a slug of the name.
    pub id: String,
    /// The id as written, when one was given — what `after <id>` refers to.
    pub task_id: Option<String>,
    pub name: String,
    pub section: String,
    pub tags: Vec<Status>,
    pub milestone: bool,
    /// Days from the start of the chart.
    pub start_day: i64,
    pub end_day: i64,
    pub duration_days: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_is_written_as_the_keyword_it_was_read_from() {
        for status in [
            Status::Done,
            Status::Active,
            Status::Crit,
            Status::Milestone,
        ] {
            assert_eq!(Status::from_keyword(status.token()), Some(status));
        }
    }

    #[test]
    fn a_status_keyword_is_read_whatever_case_it_is_written_in() {
        assert_eq!(Status::from_keyword("DONE"), Some(Status::Done));
        assert_eq!(Status::from_keyword("Crit"), Some(Status::Crit));
    }

    #[test]
    fn a_word_that_names_no_status_is_not_one() {
        assert_eq!(Status::from_keyword("urgent"), None);
    }
}
