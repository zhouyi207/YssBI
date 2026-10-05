use std::collections::{BTreeMap, BTreeSet};

use tantivy::query::{
    BooleanQuery, BoostQuery, ConstScoreQuery, Occur, Query, TermQuery, TermSetQuery,
};
use tantivy::schema::{
    FAST, Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::tokenizer::{TokenStream, Tokenizer};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument, Term, doc};
use yss_harness_contract::{
    KnowledgeChunkId, KnowledgeDocumentId, KnowledgeIndexChunk, KnowledgeIndexFailure,
    KnowledgeIndexFuture, KnowledgeIndexHit, KnowledgeIndexQuery, KnowledgeIndexReaderPort,
};

use crate::collector::{BestDocuments, DOCUMENT_ORDINAL};
use crate::tokenizer::{MixedTextTokenizer, NAME};

#[derive(Clone, Copy)]
struct Fields {
    document_id: Field,
    chunk_id: Field,
    ordinal: Field,
    title: Field,
    metadata: Field,
    body: Field,
}

#[derive(Clone)]
pub(super) struct KnowledgeIndex {
    reader: IndexReader,
    fields: Fields,
}

impl KnowledgeIndex {
    pub fn build(chunks: Vec<KnowledgeIndexChunk>) -> tantivy::Result<Self> {
        let mut schema = Schema::builder();
        let text = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(NAME)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        let fields = Fields {
            document_id: schema.add_text_field("document_id", STRING | STORED),
            chunk_id: schema.add_text_field("chunk_id", STORED),
            ordinal: schema.add_u64_field(DOCUMENT_ORDINAL, FAST),
            title: schema.add_text_field("title", text.clone()),
            metadata: schema.add_text_field("metadata", text.clone()),
            body: schema.add_text_field("body", text.set_stored()),
        };
        let index = Index::create_in_ram(schema.build());
        index.tokenizers().register(NAME, MixedTextTokenizer);
        // This controls segment flushing, not a limit on source or agent task size.
        let mut writer = index.writer_with_num_threads::<TantivyDocument>(1, 16_000_000)?;
        let ordinals: BTreeMap<_, _> = chunks
            .iter()
            .map(|chunk| chunk.document_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .enumerate()
            .map(|(ordinal, id)| (id, ordinal as u64))
            .collect();
        for chunk in chunks {
            writer.add_document(tantivy::doc!(
                fields.document_id => chunk.document_id.as_str(),
                fields.chunk_id => chunk.chunk_id.as_str(),
                fields.ordinal => ordinals[&chunk.document_id],
                fields.title => chunk.title,
                fields.metadata => chunk.scopes.into_iter().chain(chunk.tags).collect::<Vec<_>>().join(" "),
                fields.body => chunk.body,
            ))?;
        }
        writer.commit()?;
        writer.wait_merging_threads()?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self { reader, fields })
    }

    fn query(&self, request: KnowledgeIndexQuery) -> tantivy::Result<Vec<KnowledgeIndexHit>> {
        if request.limit == 0 || request.documents.is_empty() {
            return Ok(Vec::new());
        }
        let mut tokenizer = MixedTextTokenizer;
        let mut stream = tokenizer.token_stream(&request.text);
        let mut terms = BTreeSet::new();
        while let Some(token) = stream.next() {
            terms.insert(token.text.clone());
        }
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let fields = self.fields;
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for term in terms {
            for (field, boost) in [
                (fields.title, 4.0),
                (fields.metadata, 2.0),
                (fields.body, 1.0),
            ] {
                clauses.push((
                    Occur::Should,
                    Box::new(BoostQuery::new(
                        Box::new(TermQuery::new(
                            Term::from_field_text(field, &term),
                            IndexRecordOption::WithFreqs,
                        )),
                        boost,
                    )),
                ));
            }
        }
        let allowed = TermSetQuery::new(
            request
                .documents
                .iter()
                .map(|id| Term::from_field_text(fields.document_id, id.as_str())),
        );
        let query = BooleanQuery::new(vec![
            (Occur::Must, Box::new(BooleanQuery::new(clauses))),
            (
                Occur::Must,
                Box::new(ConstScoreQuery::new(Box::new(allowed), 0.0)),
            ),
        ]);
        let searcher = self.reader.searcher();
        let selected = searcher.search(&query, &BestDocuments(request.limit))?;
        let mut snippets = SnippetGenerator::create(&searcher, &query, fields.body)?;
        snippets.set_max_num_chars(480);
        selected
            .into_iter()
            .map(|(score, address)| {
                let document: TantivyDocument = searcher.doc(address)?;
                let body = text_value(&document, fields.body)?;
                let snippet = snippets.snippet(body);
                let excerpt = if snippet.fragment().is_empty() {
                    body.chars().take(480).collect()
                } else {
                    snippet.fragment().to_owned()
                };
                Ok(KnowledgeIndexHit {
                    document_id: KnowledgeDocumentId::try_new(text_value(
                        &document,
                        fields.document_id,
                    )?)
                    .map_err(|_| invalid_document())?,
                    chunk_id: KnowledgeChunkId::try_new(text_value(&document, fields.chunk_id)?)
                        .map_err(|_| invalid_document())?,
                    excerpt,
                    score,
                })
            })
            .collect()
    }
}

impl KnowledgeIndexReaderPort for KnowledgeIndex {
    fn search(
        &self,
        query: KnowledgeIndexQuery,
    ) -> KnowledgeIndexFuture<'_, Vec<KnowledgeIndexHit>> {
        let snapshot = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || snapshot.query(query))
                .await
                .map_err(|_| KnowledgeIndexFailure)?
                .map_err(|_| KnowledgeIndexFailure)
        })
    }
}

fn text_value(document: &TantivyDocument, field: Field) -> tantivy::Result<&str> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .ok_or_else(invalid_document)
}

fn invalid_document() -> tantivy::TantivyError {
    tantivy::TantivyError::InvalidArgument("invalid knowledge index document".into())
}
