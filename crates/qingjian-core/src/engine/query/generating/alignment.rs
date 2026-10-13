//! 对已生成的纯汉字句子校验全拼／简拼覆盖，不生成新的候选或枚举句子。

use pinyin::ToPinyinMulti;
use qingjian_dictionary::{SyllablePattern, canonical_syllable};

use crate::fuzzy::FuzzyRules;
use crate::parser::{INITIALS, MAX_SYLLABLE_LEN};

/// 未收录的字返回 `None`，不以缺少读音证据为由剔除候选。
pub(in crate::engine::query) fn matches_keys(
    keys: &str,
    text: &str,
    fuzzy: &FuzzyRules,
) -> Option<bool> {
    if !keys.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let readings: Option<Vec<_>> = text.chars().map(|ch| ch.to_pinyin_multi()).collect();
    let readings = readings?;
    let mut reachable = vec![false; keys.len() + 1];
    let mut next = vec![false; keys.len() + 1];
    reachable[0] = true;
    for variants in readings {
        next.fill(false);
        for reading in variants {
            // 与 lexicon 词库生成规则一致：成音节的 n / ng（如“嗯”）用 en 输入。
            let plain = match reading.plain() {
                "n" | "ng" => "en".to_owned(),
                plain => plain.replace('ü', "v"),
            };
            let expanded = fuzzy.expand(&[SyllablePattern::complete(canonical_syllable(&plain))]);
            for form in expanded.positions().into_iter().flatten() {
                let full = canonical_syllable(form.text);
                for (start, &possible) in reachable.iter().enumerate() {
                    if !possible {
                        continue;
                    }
                    for length in 1..=MAX_SYLLABLE_LEN.min(keys.len() - start) {
                        let typed = &keys[start..start + length];
                        let canonical = canonical_syllable(typed);
                        if canonical == full
                            || (INITIALS.contains(&typed) && full.starts_with(typed))
                        {
                            next[start + length] = true;
                        }
                    }
                }
            }
        }
        if !next.iter().any(|&possible| possible) {
            return Some(false);
        }
        std::mem::swap(&mut reachable, &mut next);
    }
    Some(reachable[keys.len()])
}

#[cfg(test)]
mod tests {
    use crate::fuzzy::FuzzyRules;

    fn matches_keys(keys: &str, text: &str) -> Option<bool> {
        super::matches_keys(keys, text, &FuzzyRules::default())
    }

    #[test]
    fn every_initial_must_be_consumed_by_a_matching_character() {
        assert_eq!(matches_keys("jintwyaoqu", "今天我要去"), Some(true));
        assert_eq!(matches_keys("jintwyaoqu", "今他要去"), Some(false));
        assert_eq!(matches_keys("jintwyaoqu", "今天你要去"), Some(false));
        assert_eq!(matches_keys("jintwyaoqu", "今天我还要去"), Some(false));
    }

    #[test]
    fn valid_other_segmentation_and_polyphonic_readings_are_preserved() {
        assert_eq!(
            matches_keys("nijintyaoqunali", "你今天要去哪里"),
            Some(true)
        );
        assert_eq!(
            matches_keys("nijintyaoqunali", "你今天要群阿里"),
            Some(true)
        );
        assert_eq!(matches_keys("yinh", "银行"), Some(true));
        assert_eq!(matches_keys("yinxing", "银行"), Some(true));
        assert_eq!(matches_keys("nvhai", "女孩"), Some(true));
        assert_eq!(matches_keys("jvzi", "句子"), Some(true));
    }

    #[test]
    fn syllabic_n_uses_the_product_dictionary_spelling() {
        assert_eq!(matches_keys("en", "嗯"), Some(true));
        assert_eq!(matches_keys("enjintwyaoqu", "嗯今天我要去"), Some(true));
        assert_eq!(matches_keys("enjintwyaoqu", "嗯今他要去"), Some(false));
    }

    #[test]
    fn generated_en_reading_can_lead_a_different_dictionary_candidate() {
        use std::sync::Arc;

        use qingjian_dictionary::Dictionary;

        use crate::candidate::CandidateKind;
        use crate::engine::Engine;

        use super::super::fixture::Generates;

        let dictionary = Dictionary::parse("恩今天我要去\ten jin tian wo yao qu\t10000\n").unwrap();
        let mut engine = Engine::new(dictionary).with_sentence_scorer(
            Box::new(Generates {
                texts: vec!["嗯今天我要去", "嗯今他要去"],
                calls: Arc::default(),
            }),
            None,
            None,
            None,
        );
        engine.set_input("enjintwyaoqu");
        let query = engine.query().unwrap();
        assert_eq!(query.candidates.items[0].text, "嗯今天我要去");
        assert_eq!(query.candidates.items[0].kind, CandidateKind::Generated);
        assert!(
            query
                .candidates
                .items
                .iter()
                .any(|c| c.text == "恩今天我要去")
        );
        assert!(
            !query
                .candidates
                .items
                .iter()
                .any(|c| c.text == "嗯今他要去")
        );
    }

    #[test]
    fn missing_reading_evidence_does_not_reject_a_candidate() {
        assert_eq!(matches_keys("test", "🙂"), None);
    }

    #[test]
    fn configured_fuzzy_initials_are_not_rejected() {
        assert_eq!(matches_keys("ns", "老师"), Some(false));
        let mut fuzzy = FuzzyRules::default();
        assert!(fuzzy.enable("n_l"));
        assert_eq!(super::matches_keys("ns", "老师", &fuzzy), Some(true));
    }
}
