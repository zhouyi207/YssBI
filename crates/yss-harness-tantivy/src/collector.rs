//! Group before limiting, so a long document cannot crowd out other sources.

use std::collections::BTreeMap;
use tantivy::collector::{Collector, SegmentCollector};
use tantivy::columnar::Column;
use tantivy::{DocAddress, DocId, Score, SegmentOrdinal, SegmentReader};

pub(super) const DOCUMENT_ORDINAL: &str = "document_ordinal";
type BestPassages = BTreeMap<u64, (Score, DocAddress)>;

pub(super) struct BestDocuments(pub usize);

pub(super) struct SegmentDocuments {
    ordinal: SegmentOrdinal,
    documents: Column<u64>,
    best: BestPassages,
}

impl Collector for BestDocuments {
    type Fruit = Vec<(Score, DocAddress)>;
    type Child = SegmentDocuments;

    fn for_segment(
        &self,
        ordinal: SegmentOrdinal,
        segment: &SegmentReader,
    ) -> tantivy::Result<Self::Child> {
        Ok(SegmentDocuments {
            ordinal,
            documents: segment.fast_fields().u64(DOCUMENT_ORDINAL)?,
            best: BestPassages::new(),
        })
    }

    fn requires_scoring(&self) -> bool {
        true
    }

    fn merge_fruits(&self, fruits: Vec<BestPassages>) -> tantivy::Result<Self::Fruit> {
        let mut best = BestPassages::new();
        for fruit in fruits {
            for (document, candidate) in fruit {
                retain_best(&mut best, document, candidate);
            }
        }
        let mut ranked: Vec<_> = best.into_iter().collect();
        ranked.sort_by(|(left_id, left), (right_id, right)| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| left_id.cmp(right_id))
        });
        Ok(ranked
            .into_iter()
            .take(self.0)
            .map(|(_, hit)| hit)
            .collect())
    }
}

impl SegmentCollector for SegmentDocuments {
    type Fruit = BestPassages;

    fn collect(&mut self, doc: DocId, score: Score) {
        if let Some(document) = self.documents.first(doc) {
            retain_best(
                &mut self.best,
                document,
                (score, DocAddress::new(self.ordinal, doc)),
            );
        }
    }

    fn harvest(self) -> Self::Fruit {
        self.best
    }
}

fn retain_best(best: &mut BestPassages, document: u64, candidate: (Score, DocAddress)) {
    let current = best.entry(document).or_insert(candidate);
    if candidate.0 > current.0 || (candidate.0 == current.0 && candidate.1 < current.1) {
        *current = candidate;
    }
}
