//! Resolve ordered exact replacements against one unpublished body, then use the original writer.
use super::*;

pub(super) fn apply(
    mut markdown: String,
    edit: ResourceEdit,
    control: &CapabilityControl,
) -> Result<(String, Vec<DocumentTextChange>)> {
    let mut changes = Vec::new();
    match edit {
        ResourceEdit::WriteDocument {
            markdown: replacement,
        } => {
            changes.push(change(
                0,
                markdown.chars().count(),
                replacement.chars().count(),
            ));
            markdown = replacement;
        }
        ResourceEdit::AppendDocument { text } => {
            if text.is_empty() {
                return Err(invalid("text"));
            }
            changes.push(change(markdown.chars().count(), 0, text.chars().count()));
            markdown.push_str(&text);
        }
        ResourceEdit::ReplaceDocumentText { replacements } => {
            for replacement in replacements {
                control.check()?;
                let old_text = replacement.old_text;
                if old_text.is_empty() {
                    return Err(text_failure(
                        "empty_text",
                        "Copy a nonempty passage from the inspected document into oldText.",
                    ));
                }
                let Some(start) = markdown.find(&old_text) else {
                    return Err(text_failure(
                        "text_not_found",
                        "Inspect the relevant passage and copy its exact current text into oldText. The batch was not applied.",
                    ));
                };
                if markdown.rfind(&old_text) != Some(start) {
                    return Err(text_failure(
                        "text_not_unique",
                        "Include more surrounding text in oldText to identify one occurrence. The batch was not applied.",
                    ));
                }
                changes.push(change(
                    markdown[..start].chars().count(),
                    old_text.chars().count(),
                    replacement.new_text.chars().count(),
                ));
                markdown.replace_range(start..start + old_text.len(), &replacement.new_text);
            }
        }
        _ => return Err(invalid("edit.kind")),
    }
    if markdown.len() > yss_project_model::file::MAX_FILE_BYTES {
        return Err(invalid("markdown").with_detail("reason", "document_too_large").with_detail("nextStep", "Keep the complete document within the file owner's size limit; reading pages does not change that limit."));
    }
    Ok((markdown, changes))
}

fn change(start: usize, old_length: usize, new_length: usize) -> DocumentTextChange {
    DocumentTextChange {
        before: character_range(start..start + old_length),
        after: character_range(start..start + new_length),
    }
}

fn text_failure(reason: &str, next_step: &str) -> CapabilityFailure {
    invalid("oldText")
        .with_detail("reason", reason)
        .with_detail("nextStep", next_step)
}
