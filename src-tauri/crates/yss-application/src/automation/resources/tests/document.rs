use super::*;
use serde_json::json;
use yss_harness_contract::model::*;

fn read(
    f: &mut Fixture,
    input: DocumentReadInput,
    version: Option<ResourceVersion>,
) -> Result<DocumentReadResult> {
    match f.call(AutomationCapabilityRequest::ReadDocument(
        DocumentReadRequest { input, version },
    ))? {
        AutomationCapabilityResult::DocumentRead(value) => Ok(value),
        _ => panic!("expected document read"),
    }
}

#[test]
fn document_structure_reads_long_writes_and_append_preserve_unicode_and_saved_content() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Doc {
        name: "Long report".into(),
    });
    let document = DocumentResourceRef::new(resource.id.clone());
    let original = format!(
        "# 报告\n\n## 结果\n结果=1🙂\n\n## 结果\n结果=2🙂\n\n```md\n# 代码里的标题\n```\n\n## 长文\n{}",
        "尾🙂".repeat(20000)
    );
    assert!(original.len() > 65_536);
    let created = f.edit(
        &resource,
        ResourceEdit::WriteDocument {
            markdown: original.clone(),
        },
    );
    let facts = created.document_edit.as_ref().unwrap();
    assert_eq!(
        facts.changes[0].before,
        DocumentCharacterRange { start: 0, end: 0 }
    );
    assert_eq!(facts.character_count, original.chars().count());
    assert!(facts.dirty);
    let outline =
        DocumentReadInput::Outline(serde_json::from_value(json!({"document":document})).unwrap());
    let structure = read(&mut f, outline.clone(), None).unwrap();
    let DocumentReadContent::Outline {
        headings,
        character_count,
        ..
    } = &structure.content
    else {
        panic!()
    };
    assert_eq!(*character_count, original.chars().count());
    assert_eq!(headings.len(), 4);
    assert_eq!(headings[1].title, headings[2].title);
    assert_ne!(headings[1].section, headings[2].section);
    let section = headings[2].section.clone();
    let visible =
        model::capability_result(&AutomationCapabilityResult::DocumentRead(structure.clone()))
            .unwrap();
    assert!(visible["payload"].get("version").is_none());
    let text =
        DocumentReadInput::Text(serde_json::from_value(json!({"document":document})).unwrap());
    let first = read(&mut f, text, Some(structure.version.clone())).unwrap();
    let DocumentReadContent::Text {
        markdown,
        page,
        complete,
        ..
    } = first.content
    else {
        panic!()
    };
    assert_eq!(markdown.chars().count(), 8192);
    assert_eq!(page.next_offset, Some(8192));
    assert!(!complete);
    let scoped = DocumentReadInput::Text(
        serde_json::from_value(json!({"document":document,"section":section})).unwrap(),
    );
    let second = read(&mut f, scoped.clone(), Some(structure.version.clone())).unwrap();
    let DocumentReadContent::Text {
        markdown, complete, ..
    } = second.content
    else {
        panic!()
    };
    assert!(complete);
    assert!(markdown.starts_with("## 结果\n结果=2🙂"));
    assert!(markdown.contains("# 代码里的标题"));
    assert!(!markdown.contains("## 长文"));
    let position = original.chars().position(|c| c == '🙂').unwrap();
    let scalar = read(
        &mut f,
        DocumentReadInput::Text(
            serde_json::from_value(
                json!({"document":document,"range":{"start":position,"end":position+1}}),
            )
            .unwrap(),
        ),
        None,
    )
    .unwrap();
    assert!(
        matches!(scalar.content, DocumentReadContent::Text { markdown, complete:true, .. } if markdown == "🙂")
    );
    let searched = read(&mut f, DocumentReadInput::Search(serde_json::from_value(json!({"document":document,"query":"结果=","offset":1,"limit":1,"contextCharacters":4})).unwrap()), None).unwrap();
    let DocumentReadContent::Search { matches, page } = searched.content else {
        panic!()
    };
    assert_eq!(page.total, Some(2));
    assert_eq!(matches[0].section, Some(section));
    assert!(matches[0].context.contains("结果=2🙂"));
    let appended = "\n\n## 结论\n待核对。";
    let appended_receipt = f.edit(
        &resource,
        ResourceEdit::AppendDocument {
            text: appended.into(),
        },
    );
    let change = &appended_receipt.document_edit.unwrap().changes[0];
    assert_eq!(change.before.start, original.chars().count());
    assert_eq!(change.before.start, change.before.end);
    assert_eq!(
        change.after.end,
        original.chars().count() + appended.chars().count()
    );
    assert_eq!(
        read(&mut f, scoped, Some(structure.version.clone()))
            .unwrap_err()
            .code,
        CapabilityFailureCode::RevisionConflict
    );
    let replaced = f.edit(
        &resource,
        ResourceEdit::ReplaceDocumentText {
            replacements: vec![DocumentTextReplacement {
                old_text: "结果=2🙂".into(),
                new_text: "结果=3🙂".into(),
            }],
        },
    );
    assert_eq!(replaced.document_edit.as_ref().unwrap().changes.len(), 1);
    let current = read(&mut f, outline.clone(), None).unwrap();
    let publications = f.publications.len();
    let oversized = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: current.version.clone(),
                edit: ResourceEdit::WriteDocument {
                    markdown: "x".repeat(yss_project_model::file::MAX_FILE_BYTES + 1),
                },
            },
        ))
        .unwrap_err();
    assert_eq!(oversized.details["reason"], "document_too_large");
    assert_eq!(f.publications.len(), publications);
    assert_eq!(
        read(&mut f, outline.clone(), None).unwrap().version,
        current.version
    );
    let invalid = DocumentReadInput::Text(
        serde_json::from_value(json!({"document":document,"range":{"start":0,"end":usize::MAX}}))
            .unwrap(),
    );
    assert_eq!(
        read(&mut f, invalid, None).unwrap_err().code,
        CapabilityFailureCode::InvalidRequest
    );
    f.save(&resource);
    assert!(!read(&mut f, outline, None).unwrap().dirty);
    assert_eq!(
        std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
        original.replace("结果=2🙂", "结果=3🙂") + appended
    );
    // JSON escaping can enlarge search context beyond the result budget. Return an honest smaller page.
    f.edit(
        &resource,
        ResourceEdit::WriteDocument {
            markdown: "\0".repeat(5000),
        },
    );
    let bounded = read(&mut f, DocumentReadInput::Search(serde_json::from_value(json!({"document":document,"query":"\0".repeat(4096),"limit":100,"contextCharacters":500})).unwrap()), None).unwrap();
    let DocumentReadContent::Search { matches, page } = &bounded.content else {
        panic!()
    };
    assert_eq!(page.total, Some(905));
    assert!(!matches.is_empty() && matches.len() < 100);
    assert_eq!(page.next_offset, Some(matches.len()));
    AutomationCapabilityResult::DocumentRead(bounded)
        .validate_budget(MAX_CAPABILITY_RESULT_BYTES)
        .unwrap();
}

#[test]
fn markdown_text_edits_preserve_unread_content_and_reject_ambiguous_batches_atomically() {
    let mut f = Fixture::new();
    let resource = f.create(ResourceCreation::Doc {
        name: "Exact text".into(),
    });
    let original = format!("开头\n结果=1🙂\n{}", "尾".repeat(30000));
    f.edit(
        &resource,
        ResourceEdit::WriteDocument {
            markdown: original.clone(),
        },
    );
    f.save(&resource);
    let input = DocumentReadInput::Text(
        serde_json::from_value(json!({"document":DocumentResourceRef::new(resource.id.clone())}))
            .unwrap(),
    );
    let page = read(&mut f, input, None).unwrap();
    assert!(
        matches!(&page.content, DocumentReadContent::Text { markdown, page, complete:false, .. } if markdown.chars().count() == 8192 && page.next_offset == Some(8192))
    );
    f.edit(
        &resource,
        ResourceEdit::ReplaceDocumentText {
            replacements: vec![
                DocumentTextReplacement {
                    old_text: "结果=1🙂".into(),
                    new_text: "结果=2🙂".into(),
                },
                DocumentTextReplacement {
                    old_text: "结果=2🙂".into(),
                    new_text: "结果=3🙂".into(),
                },
            ],
        },
    );
    let expected = original.replace("结果=1🙂", "结果=3🙂");
    let current = f.inspect(&resource);
    let publications = f.publications.len();
    for (old_text, reason) in [
        ("", "empty_text"),
        ("absent", "text_not_found"),
        ("尾尾", "text_not_unique"),
    ] {
        let failure = f
            .call(AutomationCapabilityRequest::EditResource(
                EditResourceRequest {
                    resource: resource.clone(),
                    version: current.version.clone(),
                    edit: ResourceEdit::ReplaceDocumentText {
                        replacements: vec![
                            DocumentTextReplacement {
                                old_text: "结果=3🙂".into(),
                                new_text: "不会提交".into(),
                            },
                            DocumentTextReplacement {
                                old_text: old_text.into(),
                                new_text: "replacement".into(),
                            },
                        ],
                    },
                },
            ))
            .unwrap_err();
        assert_eq!(failure.code, CapabilityFailureCode::InvalidRequest);
        assert_eq!(failure.details["reason"], reason);
        assert_eq!(f.inspect(&resource).version, current.version);
    }
    let stale = f
        .call(AutomationCapabilityRequest::EditResource(
            EditResourceRequest {
                resource: resource.clone(),
                version: page.version,
                edit: ResourceEdit::ReplaceDocumentText {
                    replacements: vec![DocumentTextReplacement {
                        old_text: "结果=3🙂".into(),
                        new_text: "stale".into(),
                    }],
                },
            },
        ))
        .unwrap_err();
    assert_eq!(stale.code, CapabilityFailureCode::RevisionConflict);
    assert_eq!(f.publications.len(), publications);
    f.save(&resource);
    assert_eq!(
        std::fs::read_to_string(f.directory.join("project").join(&resource.id)).unwrap(),
        expected
    );
}
