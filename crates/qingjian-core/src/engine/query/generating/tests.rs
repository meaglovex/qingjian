//! 混合简拼不能被缺字或错读音的自由生成结果挤掉；混输仍有兜底。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use qingjian_dictionary::Dictionary;

use super::fixture::Generates;
use super::resolved_abbreviation;
use crate::candidate::{Candidate, CandidateKind};
use crate::engine::Engine;
use crate::parser::{self, Segmentation, Syllable};

const KEYS: &str = "jintwyaoqu";
const DICT: &str = "今天\tjin tian\t10000\n我\two\t9000\n要\tyao\t8000\n去\tqu\t7000\n";

fn candidate(text: &str, readings: &[&str]) -> Candidate {
    Candidate {
        text: text.to_owned(),
        kind: CandidateKind::Sentence,
        syllables: readings.iter().map(|s| (*s).to_owned()).collect(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

#[test]
fn a_complete_alignment_is_required_to_protect_abbreviations() {
    let best = parser::segment(KEYS).unwrap().remove(0);
    let right = candidate("今天我要去", &["jin", "tian", "wo", "yao", "qu"]);
    assert!(resolved_abbreviation(
        &best,
        KEYS,
        false,
        [&right].into_iter()
    ));
    for wrong in [
        candidate("今他要去", &["jin", "ta", "yao", "qu"]),
        candidate("今天你要去", &["jin", "tian", "ni", "yao", "qu"]),
        candidate("今天我要去", &["jing", "tian", "wo", "yao", "qu"]),
    ] {
        assert!(!resolved_abbreviation(
            &best,
            KEYS,
            false,
            [&wrong].into_iter()
        ));
    }
    assert!(!resolved_abbreviation(
        &best,
        KEYS,
        true,
        [&right].into_iter()
    ));
    assert!(!resolved_abbreviation(
        &best,
        "jintwyaoquv",
        false,
        [&right].into_iter()
    ));
}

#[test]
fn an_unfinished_syllable_is_not_an_initial_abbreviation() {
    let best = Segmentation {
        syllables: vec![Syllable::complete("wo"), Syllable::partial("zho")],
    };
    let right = candidate("我中", &["wo", "zhong"]);
    assert!(!resolved_abbreviation(
        &best,
        "wozho",
        false,
        [&right].into_iter()
    ));
}

#[test]
fn generated_chinese_does_not_displace_a_resolved_abbreviation() {
    for asynchronous in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let scorer = Box::new(Generates {
            texts: vec!["今他要去", "今天你要去"],
            calls: calls.clone(),
        });
        let engine = Engine::new(Dictionary::parse(DICT).unwrap());
        let mut engine = if asynchronous {
            engine.with_async_sentence_scorer(scorer, None, None, None)
        } else {
            engine.with_sentence_scorer(scorer, None, None, None)
        };
        engine.set_input(KEYS);
        engine.query().unwrap();
        if asynchronous {
            assert!(engine.request_rescoring());
            let started = Instant::now();
            while !engine.poll_rescoring() {
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        for _ in 0..2 {
            let query = engine.query().unwrap();
            assert_eq!(query.candidates.items[0].text, "今天我要去");
            assert!(
                !query
                    .candidates
                    .items
                    .iter()
                    .any(|c| c.kind == CandidateKind::Generated)
            );
        }
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn a_whole_dictionary_word_also_protects_the_abbreviation() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut engine =
        Engine::new(Dictionary::parse("今天我要去\tjin tian wo yao qu\t10000\n").unwrap())
            .with_sentence_scorer(
                Box::new(Generates {
                    texts: vec!["今他要去"],
                    calls,
                }),
                None,
                None,
                None,
            );
    engine.set_input(KEYS);
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "今天我要去");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Chinese);
}

#[test]
fn generation_remains_available_without_a_complete_word_path() {
    let mut engine = Engine::new(Dictionary::parse("我\two\t10000\n").unwrap())
        .with_sentence_scorer(
            Box::new(Generates {
                texts: vec!["今天我要去"],
                calls: Arc::default(),
            }),
            None,
            None,
            None,
        );
    engine.set_input(KEYS);
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "今天我要去");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Generated);
}

#[test]
fn generated_mixed_text_can_still_lead_a_phonetic_word_path() {
    let dict = "用\tyong\t10000\n的\tde\t9000\n哦\to\t8000\n测\tce\t7000\n可\tke\t6000\n人\tren\t5000\n部署\tbu shu\t4000\n很\then\t3000\n方便\tfang bian\t2000\n";
    let mut engine = Engine::new(Dictionary::parse(dict).unwrap()).with_sentence_scorer(
        Box::new(Generates {
            texts: vec!["用docker部署很方便"],
            calls: Arc::default(),
        }),
        None,
        None,
        None,
    );
    engine.set_input("yongdockerbushuhenfangbian");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "用docker部署很方便");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Generated);
}

#[test]
fn generated_chinese_can_use_another_valid_pinyin_segmentation() {
    let dict = "你\tni\t10000\n今天\tjin tian\t9000\n要\tyao\t8000\n群\tqun\t7000\n阿\ta\t6000\n里\tli\t5000\n";
    let mut engine = Engine::new(Dictionary::parse(dict).unwrap()).with_sentence_scorer(
        Box::new(Generates {
            texts: vec!["你今天要去哪里"],
            calls: Arc::default(),
        }),
        None,
        None,
        None,
    );
    engine.set_input("nijintyaoqunali");
    let query = engine.query().unwrap();
    assert!(
        query
            .candidates
            .items
            .iter()
            .any(|c| c.text == "你今天要群阿里")
    );
    assert_eq!(query.candidates.items[0].text, "你今天要去哪里");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Generated);
}
