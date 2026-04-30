pub use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32String};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Default)]
pub(crate) enum Status {
    #[default]
    Unchanged,
    Update,
    Rescore,
}

#[derive(Debug)]
pub struct MultiPattern {
    cols: Vec<(Pattern, Status)>,
    weights: Vec<u32>,
}

impl Clone for MultiPattern {
    fn clone(&self) -> Self {
        Self {
            cols: self.cols.clone(),
            weights: self.weights.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.cols.clone_from(&source.cols)
    }
}

impl MultiPattern {
    /// Creates a multi pattern with `columns` empty column patterns.
    pub fn new(columns: usize) -> Self {
        Self {
            cols: vec![Default::default(); columns],
            weights: vec![1; columns],
        }
    }

    /// Reparses a column. By specifying `append` the caller promises that text passed
    /// to the previous `reparse` invocation is a prefix of `new_text`. This enables
    /// additional optimizations but can lead to missing matches if an incorrect value
    /// is passed.
    pub fn reparse(
        &mut self,
        column: usize,
        new_text: &str,
        case_matching: CaseMatching,
        normalization: Normalization,
        append: bool,
    ) {
        let old_status = self.cols[column].1;
        if append
            && old_status != Status::Rescore
            && self.cols[column]
                .0
                .atoms
                .last()
                .map_or(true, |last| !last.negative)
        {
            self.cols[column].1 = Status::Update;
        } else {
            self.cols[column].1 = Status::Rescore;
        }
        self.cols[column]
            .0
            .reparse(new_text, case_matching, normalization);
    }

    pub fn column_pattern(&self, column: usize) -> &Pattern {
        &self.cols[column].0
    }

    pub(crate) fn status(&self) -> Status {
        self.cols
            .iter()
            .map(|&(_, status)| status)
            .max()
            .unwrap_or(Status::Unchanged)
    }

    pub(crate) fn reset_status(&mut self) {
        for (_, status) in &mut self.cols {
            *status = Status::Unchanged
        }
    }
    pub fn set_weight(&mut self, column: usize, weight: u32) {
        self.weights[column] = weight
    }

    pub fn score(&self, haystack: &[Utf32String], matcher: &mut Matcher) -> Option<u32> {
        let mut score = 0;
        for (i, ((pattern, _), haystack)) in self.cols.iter().zip(haystack).enumerate() {
            let weight = *self.weights.get(i).unwrap_or(&1);

            score += pattern.score(haystack.slice(..), matcher)? * weight / 100
        }
        Some(score)
    }

    pub fn is_empty(&self) -> bool {
        self.cols.iter().all(|(pat, _)| pat.atoms.is_empty())
    }
}
