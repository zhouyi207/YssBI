//! Stable Unicode passages and citation identities shared by indexing and reads.
use super::{KnowledgeChunkId, KnowledgeError, SourceHash};

const PASSAGE_CHARS: usize = 1_200;
const PASSAGE_OVERLAP_CHARS: usize = 120;

#[derive(Clone, Copy)]
pub(super) struct Passage<'a> {
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
}

pub(super) fn passages(body: &str) -> impl Iterator<Item = Passage<'_>> {
    let mut start = 0;
    let mut finished = false;
    std::iter::from_fn(move || {
        if finished {
            return None;
        }
        let rest = &body[start..];
        let length = rest
            .char_indices()
            .nth(PASSAGE_CHARS)
            .map(|(index, _)| index);
        let end = length.map_or(body.len(), |length| start + length);
        let passage = Passage {
            start,
            end,
            text: &body[start..end],
        };
        finished = end == body.len();
        if !finished {
            start = end
                - passage
                    .text
                    .chars()
                    .rev()
                    .take(PASSAGE_OVERLAP_CHARS)
                    .map(char::len_utf8)
                    .sum::<usize>();
        }
        Some(passage)
    })
}

pub(super) fn chunk_id(
    document_id: &str,
    source_hash: &SourceHash,
    passage: Passage<'_>,
) -> Result<KnowledgeChunkId, KnowledgeError> {
    let digest = yss_canonical_hash::hash_canonical(
        "yssbi.knowledge.chunk.v2",
        &(
            document_id,
            source_hash,
            passage.start,
            passage.end,
            passage.text,
        ),
    )
    .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let suffix = hex::encode(&digest[..16]);
    KnowledgeChunkId::try_new(format!("chunk-{suffix}"))
        .map_err(|_| KnowledgeError::SourceIntegrity)
}
