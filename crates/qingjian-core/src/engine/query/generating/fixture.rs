//! 记录调用次数的固定生成器，用于简拼候选回归。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::sentence::SentenceScorer;

pub(super) struct Generates {
    pub(super) texts: Vec<&'static str>,

    pub(super) calls: Arc<AtomicUsize>,
}

impl SentenceScorer for Generates {
    fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
        vec![0.0; texts.len()]
    }

    fn generate(&self, _keys: &str, _beam: usize, _max_chars: usize) -> Vec<String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.texts.iter().map(|text| (*text).to_owned()).collect()
    }
}
