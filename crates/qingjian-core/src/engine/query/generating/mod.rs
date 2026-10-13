//! 词图读不通整段输入时的生成兜底，以及已解析简拼的对齐保护。

use qingjian_dictionary::canonical_syllable;

use crate::candidate::{Candidate, CandidateKind};
use crate::parser::{INITIALS, Segmentation};
use crate::sentence;

mod alignment;
mod candidates;

pub(super) use alignment::matches_keys;

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod tests;

/// 原样覆盖全部按键的简拼已有逐音节对齐候选，可以检查生成句是否漏字或错读音。
pub(super) fn resolved_abbreviation<'a>(
    best: &Segmentation,
    keys: &str,
    corrected: bool,
    candidates: impl Iterator<Item = &'a Candidate>,
) -> bool {
    if corrected
        || best.letters() != keys.len()
        || best.incomplete_count() == 0
        || best
            .syllables
            .iter()
            .any(|syllable| !syllable.complete && !INITIALS.contains(&syllable.text.as_str()))
    {
        return false;
    }
    candidates.into_iter().any(|candidate| {
        matches!(
            candidate.kind,
            CandidateKind::Chinese | CandidateKind::Sentence
        ) && candidate.syllables.len() == best.syllables.len()
            && candidate.text.chars().count() == best.syllables.len()
            && candidate.text.chars().all(sentence::is_han)
            && best
                .syllables
                .iter()
                .zip(&candidate.syllables)
                .all(|(typed, reading)| {
                    let pattern = canonical_syllable(&typed.text);
                    let reading = canonical_syllable(reading);
                    if typed.complete {
                        reading == pattern
                    } else {
                        reading.starts_with(pattern)
                    }
                })
    })
}
