//! Disposable Markdown structure and Unicode locations over the current plain-text document.
use super::DocDocument;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Debug)]
pub struct DocView<'a> {
    markdown: &'a str,
    boundaries: Vec<usize>,
    headings: Vec<DocHeading>,
    block_ends: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocHeading {
    pub title: String,
    pub level: u8,
    /// Absolute Unicode scalar offsets. The section includes its heading and descendants.
    pub start: usize,
    pub body_start: usize,
    pub end: usize,
    pub parent_index: Option<usize>,
}

#[derive(Debug)]
pub struct DocOutlinePage<'a> {
    pub headings: Vec<(usize, &'a DocHeading)>,
    pub total: usize,
}

#[derive(Debug)]
pub struct DocTextPage<'a> {
    pub markdown: &'a str,
    pub range: Range<usize>,
    pub scope: Range<usize>,
    /// Actual offset relative to scope, clamped to its end for an empty final page.
    pub offset: usize,
}

#[derive(Debug)]
pub struct DocSearchMatch<'a> {
    pub range: Range<usize>,
    pub context_range: Range<usize>,
    pub context: &'a str,
    pub heading_index: Option<usize>,
}

#[derive(Debug)]
pub struct DocSearchPage<'a> {
    pub matches: Vec<DocSearchMatch<'a>>,
    pub total: usize,
}

impl DocDocument {
    pub fn view(&self) -> DocView<'_> {
        let boundaries = self
            .0
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(self.0.len()))
            .collect::<Vec<_>>();
        let character_count = boundaries.len() - 1;
        let location = |byte| {
            boundaries
                .binary_search(&byte)
                .expect("Markdown source offsets are UTF-8 boundaries")
        };
        let mut headings: Vec<DocHeading> = Vec::new();
        let mut ancestors: Vec<usize> = Vec::new();
        let mut active_heading = None;
        let mut depth = 0;
        let mut block_ends = Vec::new();
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_MATH;
        for (event, range) in Parser::new_ext(&self.0, options).into_offset_iter() {
            match event {
                Event::Start(tag) => {
                    if let Tag::Heading { level, .. } = tag
                        && depth == 0
                    {
                        let level = match level {
                            HeadingLevel::H1 => 1,
                            HeadingLevel::H2 => 2,
                            HeadingLevel::H3 => 3,
                            HeadingLevel::H4 => 4,
                            HeadingLevel::H5 => 5,
                            HeadingLevel::H6 => 6,
                        };
                        let start = location(range.start);
                        while ancestors
                            .last()
                            .is_some_and(|index| headings[*index].level >= level)
                        {
                            headings[ancestors.pop().unwrap()].end = start;
                        }
                        let index = headings.len();
                        headings.push(DocHeading {
                            title: String::new(),
                            level,
                            start,
                            body_start: location(range.end),
                            end: character_count,
                            parent_index: ancestors.last().copied(),
                        });
                        ancestors.push(index);
                        active_heading = Some(index);
                        block_ends.push(start);
                    }
                    depth += 1;
                }
                Event::End(tag) => {
                    depth -= 1;
                    if let TagEnd::Heading(_) = tag
                        && let Some(index) = active_heading.take()
                    {
                        headings[index].body_start = location(range.end);
                    }
                    if depth == 0 {
                        block_ends.push(location(range.end));
                    }
                }
                Event::Text(value)
                | Event::Code(value)
                | Event::InlineMath(value)
                | Event::DisplayMath(value) => {
                    if let Some(index) = active_heading {
                        headings[index].title.push_str(&value);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(index) = active_heading {
                        headings[index].title.push(' ');
                    }
                }
                _ => {}
            }
        }
        block_ends.sort_unstable();
        block_ends.dedup();
        DocView {
            markdown: &self.0,
            boundaries,
            headings,
            block_ends,
        }
    }
}

impl<'a> DocView<'a> {
    pub fn character_count(&self) -> usize {
        self.boundaries.len() - 1
    }
    pub fn headings(&self) -> &[DocHeading] {
        &self.headings
    }

    pub fn section(&self, index: usize, start: usize) -> Result<Range<usize>, String> {
        let heading = self
            .headings
            .get(index)
            .filter(|heading| heading.start == start)
            .ok_or("document section not found")?;
        Ok(heading.start..heading.end)
    }

    pub fn outline(
        &self,
        scope: Range<usize>,
        offset: usize,
        limit: usize,
    ) -> Result<DocOutlinePage<'_>, String> {
        self.validate_range(&scope)?;
        if limit == 0 {
            return Err("empty document outline page".into());
        }
        let headings = self
            .headings
            .iter()
            .enumerate()
            .filter(|(_, heading)| scope.contains(&heading.start))
            .collect::<Vec<_>>();
        let total = headings.len();
        Ok(DocOutlinePage {
            headings: headings.into_iter().skip(offset).take(limit).collect(),
            total,
        })
    }

    /// Reads only the requested scope. Prefer a nearby complete block when it uses at least half the page.
    pub fn read(
        &self,
        scope: Range<usize>,
        offset: usize,
        limit: usize,
    ) -> Result<DocTextPage<'a>, String> {
        self.validate_range(&scope)?;
        if limit == 0 {
            return Err("empty document text page".into());
        }
        let offset = offset.min(scope.len());
        let start = scope.start + offset;
        let mut end = start.saturating_add(limit).min(scope.end);
        if end < scope.end {
            let index = self.block_ends.partition_point(|point| *point <= end);
            if let Some(boundary) = index.checked_sub(1).map(|index| self.block_ends[index])
                && boundary > start
                && boundary >= start + limit / 2
            {
                end = boundary;
            }
        }
        Ok(DocTextPage {
            markdown: self.text(start..end),
            range: start..end,
            scope,
            offset,
        })
    }

    /// Case-sensitive literal matches, including overlaps, with source-preserving context.
    pub fn search(
        &self,
        scope: Range<usize>,
        query: &str,
        offset: usize,
        limit: usize,
        context_characters: usize,
    ) -> Result<DocSearchPage<'a>, String> {
        self.validate_range(&scope)?;
        if query.is_empty() || limit == 0 {
            return Err("empty document search".into());
        }
        let source = self.text(scope.clone());
        let source_start = self.boundaries[scope.start];
        let mut byte = 0;
        let mut total = 0;
        let mut matches = Vec::new();
        while let Some(found) = source[byte..].find(query) {
            let at = byte + found;
            if total >= offset && matches.len() < limit {
                let start = self.boundaries.binary_search(&(source_start + at)).unwrap();
                let end = self
                    .boundaries
                    .binary_search(&(source_start + at + query.len()))
                    .unwrap();
                let context_range = start.saturating_sub(context_characters).max(scope.start)
                    ..end.saturating_add(context_characters).min(scope.end);
                matches.push(DocSearchMatch {
                    range: start..end,
                    context: self.text(context_range.clone()),
                    context_range,
                    heading_index: self
                        .headings
                        .partition_point(|heading| heading.start <= start)
                        .checked_sub(1),
                });
            }
            total += 1;
            byte = at + source[at..].chars().next().unwrap().len_utf8();
        }
        Ok(DocSearchPage { matches, total })
    }

    fn validate_range(&self, range: &Range<usize>) -> Result<(), String> {
        if range.start > range.end || range.end > self.character_count() {
            return Err("invalid document character range".into());
        }
        Ok(())
    }

    fn text(&self, range: Range<usize>) -> &'a str {
        &self.markdown[self.boundaries[range.start]..self.boundaries[range.end]]
    }
}

#[cfg(test)]
mod tests;
