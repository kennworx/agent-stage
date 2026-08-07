//! The parsed shape of an xy chart: two axes and a stack of series.

/// Whether a series is drawn as bars or as a curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesKind {
    Bar,
    Line,
}

impl SeriesKind {
    /// The keyword this kind is written with, which also names its legend entry.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Line => "Line",
        }
    }
}

/// One series of values, one per category.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub kind: SeriesKind,
    pub data: Vec<f64>,
}

/// A numeric span.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

/// An axis: named categories, or a numeric span, and an optional title.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Axis {
    pub title: Option<String>,
    /// Categorical labels. Mutually exclusive with `range` in practice, and the
    /// reference prefers these when a source somehow declares both.
    pub categories: Option<Vec<String>>,
    pub range: Option<Range>,
}

/// A parsed xy chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chart {
    pub title: Option<String>,
    /// Bars run left-to-right rather than bottom-up.
    pub horizontal: bool,
    pub x_axis: Axis,
    pub y_axis: Axis,
    pub series: Vec<Series>,
}

impl Chart {
    /// The value span the chart is drawn against.
    ///
    /// The parser always derives one, so the fallback is only reached by a chart
    /// assembled by hand.
    pub fn value_range(&self) -> Range {
        self.y_axis.range.unwrap_or(Range {
            min: 0.0,
            max: 100.0,
        })
    }

    /// How many points sit along the category axis.
    pub fn data_count(&self) -> usize {
        if let Some(categories) = &self.x_axis.categories {
            return categories.len();
        }
        // Without categories the first series that has any data sets the count.
        self.series
            .iter()
            .find(|series| !series.data.is_empty())
            .map_or(1, |series| series.data.len())
    }

    /// The label under each point: the categories, the numeric span stepped
    /// across, or failing both a plain count from one.
    pub fn category_labels(&self) -> Vec<String> {
        let count = self.data_count();
        if let Some(categories) = &self.x_axis.categories {
            return categories.clone();
        }
        if let Some(range) = self.x_axis.range {
            let step = if count > 1 {
                (range.max - range.min) / crate::round::count(count - 1)
            } else {
                0.0
            };
            return (0..count)
                .map(|index| format_tick(range.min + step * crate::round::count(index)))
                .collect();
        }
        (1..=count).map(|index| index.to_string()).collect()
    }
}

/// A tick value as it is written on the axis.
///
/// A whole number keeps its digits; anything else is cut to one decimal place,
/// or to none once it is large enough that a tenth adds nothing.
pub fn format_tick(value: f64) -> String {
    // `-0` reads as a mistake rather than as zero.
    let value = if value == 0.0 { 0.0 } else { value };
    if value.fract() == 0.0 && value.is_finite() {
        return format!("{value}");
    }
    if value.abs() < 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(x: Axis, series: Vec<Series>) -> Chart {
        Chart {
            x_axis: x,
            series,
            ..Chart::default()
        }
    }

    fn bar(data: &[f64]) -> Series {
        Series {
            kind: SeriesKind::Bar,
            data: data.to_vec(),
        }
    }

    #[test]
    fn a_kind_names_its_legend_entry() {
        assert_eq!(SeriesKind::Bar.token(), "Bar");
        assert_eq!(SeriesKind::Line.token(), "Line");
    }

    #[test]
    fn categories_decide_the_count_when_there_are_any() {
        let out = chart(
            Axis {
                categories: Some(vec!["a".into(), "b".into()]),
                ..Axis::default()
            },
            vec![bar(&[1.0, 2.0, 3.0])],
        );
        assert_eq!(out.data_count(), 2);
        assert_eq!(out.category_labels(), ["a", "b"]);
    }

    #[test]
    fn without_categories_the_first_series_with_data_decides() {
        let out = chart(Axis::default(), vec![bar(&[]), bar(&[1.0, 2.0, 3.0])]);
        assert_eq!(out.data_count(), 3);
        assert_eq!(out.category_labels(), ["1", "2", "3"]);
    }

    #[test]
    fn a_chart_with_nothing_in_it_still_has_one_column() {
        assert_eq!(chart(Axis::default(), Vec::new()).data_count(), 1);
        assert_eq!(chart(Axis::default(), vec![bar(&[])]).data_count(), 1);
    }

    #[test]
    fn a_numeric_axis_steps_its_labels_across_the_span() {
        let out = chart(
            Axis {
                range: Some(Range {
                    min: 0.0,
                    max: 100.0,
                }),
                ..Axis::default()
            },
            vec![bar(&[1.0, 2.0, 3.0, 4.0, 5.0])],
        );
        assert_eq!(out.category_labels(), ["0", "25", "50", "75", "100"]);
    }

    #[test]
    fn a_numeric_axis_with_one_point_does_not_divide_by_zero() {
        let out = chart(
            Axis {
                range: Some(Range {
                    min: 5.0,
                    max: 90.0,
                }),
                ..Axis::default()
            },
            vec![bar(&[1.0])],
        );
        assert_eq!(out.category_labels(), ["5"]);
    }

    #[test]
    fn a_chart_with_no_declared_span_falls_back_to_nought_to_a_hundred() {
        let out = Chart::default();
        assert_eq!(
            out.value_range(),
            Range {
                min: 0.0,
                max: 100.0
            }
        );
    }

    #[test]
    fn a_tick_is_written_as_short_as_it_can_be() {
        assert_eq!(format_tick(150.0), "150");
        assert_eq!(format_tick(-3.0), "-3");
        assert_eq!(format_tick(0.0), "0");
        assert_eq!(format_tick(-0.0), "0");
        assert_eq!(format_tick(2.5), "2.5");
        assert_eq!(format_tick(2.25), "2.2");
        assert_eq!(format_tick(1234.5), "1234");
    }
}
