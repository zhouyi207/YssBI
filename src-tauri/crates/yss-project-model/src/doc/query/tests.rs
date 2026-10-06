use super::*;

#[test]
fn outline_uses_markdown_structure_and_distinguishes_repeated_unicode_headings() {
    let document = DocDocument("前言🙂\r\n\r\n# Report\r\n\r\n## **结果**\r\n甲\r\n\r\n```markdown\r\n# not a heading\r\n```\r\n\r\n> # quoted\r\n\r\n## 结果\r\n乙\r\n\r\n### Detail\r\n丙\r\n\r\nLast\r\n====\r\n完".into());
    let view = document.view();
    let titles = view
        .headings()
        .iter()
        .map(|heading| heading.title.as_str())
        .collect::<Vec<_>>();
    assert_eq!(titles, ["Report", "结果", "结果", "Detail", "Last"]);
    assert_eq!(view.headings()[1].parent_index, Some(0));
    assert_eq!(view.headings()[3].parent_index, Some(2));
    assert_eq!(view.headings()[0].end, view.headings()[4].start);
    assert_eq!(view.headings()[1].end, view.headings()[2].start);
    let first = view.section(1, view.headings()[1].start).unwrap();
    let second = view.section(2, view.headings()[2].start).unwrap();
    assert_ne!(first, second);
    assert!(view.section(1, second.start).is_err());
    let page = view.outline(second.clone(), 0, 1).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.headings[0].0, 2);
    let text = view.read(second, 0, 16_384).unwrap();
    assert!(text.markdown.starts_with("## 结果\r\n乙"));
    assert!(text.markdown.contains("### Detail"));
    assert!(!text.markdown.contains("Last"));
    assert_eq!(text.markdown.chars().count(), text.range.len());
}

#[test]
fn text_pages_preserve_source_and_search_reports_overlapping_exact_unicode_locations() {
    let document = DocDocument(format!(
        "# 标题\n\n{}\n\n{}",
        "文🙂字".repeat(12),
        "long ".repeat(4000)
    ));
    let view = document.view();
    let scope = 0..view.character_count();
    let first = view.read(scope.clone(), 0, 64).unwrap();
    assert!(first.markdown.ends_with('\n'));
    assert!(first.range.end < 64);
    let mut rebuilt = first.markdown.to_owned();
    let mut offset = first.range.end;
    while offset < view.character_count() {
        let page = view.read(scope.clone(), offset, 8192).unwrap();
        assert!(page.range.end > page.range.start);
        rebuilt.push_str(page.markdown);
        offset = page.range.end;
    }
    assert_eq!(rebuilt, document.0);
    let empty = view.read(scope.clone(), usize::MAX, 1).unwrap();
    assert!(empty.markdown.is_empty());
    assert_eq!(empty.range, scope.end..scope.end);
    assert!(view.read(1..usize::MAX, 0, 2).is_err());
    let result = view.search(scope, "🙂字", 10, 1, 3).unwrap();
    assert_eq!(result.total, 12);
    assert_eq!(result.matches.len(), 1);
    let found = &result.matches[0];
    assert_eq!(
        document
            .0
            .chars()
            .skip(found.range.start)
            .take(found.range.len())
            .collect::<String>(),
        "🙂字"
    );
    assert_eq!(found.context.chars().count(), found.context_range.len());
    assert_eq!(found.heading_index, Some(0));
    let overlapping = DocDocument("aaaa".into());
    let view = overlapping.view();
    let page = view.search(0..4, "aa", 1, 1, 0).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.matches[0].range, 1..3);
    assert_eq!(page.matches[0].context, "aa");
}
