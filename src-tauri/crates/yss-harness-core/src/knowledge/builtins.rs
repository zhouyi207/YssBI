use super::*;

pub async fn install_builtin_statistical_knowledge(
    store: Arc<dyn KnowledgeSourceStorePort>,
    now: UnixMillis,
) -> Result<(), KnowledgeError> {
    let documents = [
        (
            "dataset-quality-review",
            "Dataset quality review / 数据质量检查",
            "Before estimation, establish measurement scales, missingness, duplicate keys, outliers, and variable semantics. Stop when the dataset changes or required semantics remain unknown.",
            vec![
                "statistics.data_quality".to_owned(),
                "statistics.missingness".to_owned(),
            ],
            vec![
                "quality".to_owned(),
                "missingness".to_owned(),
                "缺失值 重复值 异常值 测量尺度 数据质量".to_owned(),
            ],
        ),
        (
            "ols-diagnostics",
            "OLS assumptions and diagnostics / 线性回归假设与诊断",
            "OLS reporting should pair effect estimates and uncertainty with residual checks, influential-observation diagnostics, robustness checks, and explicit limitations.",
            vec![
                "statistics.regression.ols".to_owned(),
                "statistics.diagnostics".to_owned(),
            ],
            vec![
                "regression".to_owned(),
                "diagnostics".to_owned(),
                "回归 残差 诊断 不确定性 稳健性".to_owned(),
            ],
        ),
    ];
    let digest = yss_canonical_hash::hash_canonical("yssbi.knowledge.builtin.v1", &documents)
        .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let source_hash = SourceHash::try_new(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let source_id = KnowledgeSourceId::try_new("yssbi-statistical-methods")
        .map_err(|_| KnowledgeError::SourceIntegrity)?;
    let source = KnowledgeSourceRecord {
        id: source_id.clone(),
        title: "YssBI Statistical Methods".to_owned(),
        version: "1.0.0".to_owned(),
        license: "YssBI project documentation".to_owned(),
        source_hash: source_hash.clone(),
        status: KnowledgeSourceStatus::Active,
        sensitivity: SensitivityClass::Public,
        project: None,
        origin: None,
        updated_at: now,
    };
    let documents = documents
        .into_iter()
        .map(|(id, title, body, scopes, tags)| {
            Ok(KnowledgeDocumentRecord {
                id: KnowledgeDocumentId::try_new(id)
                    .map_err(|_| KnowledgeError::SourceIntegrity)?,
                source_id: source_id.clone(),
                title: title.to_owned(),
                body: body.to_owned(),
                scopes,
                tags,
                source_hash: source_hash.clone(),
                project: None,
                sensitivity: SensitivityClass::Public,
            })
        })
        .collect::<Result<Vec<_>, KnowledgeError>>()?;
    store.replace_source(&source, &documents).await?;
    Ok(())
}
