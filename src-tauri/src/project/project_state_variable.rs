#[cfg(test)]
use super::unique_name;
use super::{ProjectFilesystemError, ProjectState};

#[cfg(test)]
use yss_data_contract::{DataType, DataValue};

use crate::project::variable_tabular::normalize_variable_tabular;
use crate::variable::VariableId;
use crate::variable::VariableInstance;
#[cfg(test)]
use crate::variable::VariableScope;

impl ProjectState {
    pub(super) fn stage_variable(
        mut variable: VariableInstance,
    ) -> Result<VariableInstance, ProjectFilesystemError> {
        normalize_variable_tabular(&mut variable).map_err(|error| {
            ProjectFilesystemError::TransactionCommitFailed {
                message: error.to_string(),
            }
        })?;
        Ok(variable)
    }

    #[cfg(test)]
    pub(crate) fn add_variable(
        &self,
        name: &str,
        data_type: DataType,
        data_value: DataValue,
        description: &str,
        scope: VariableScope,
        tags: Vec<String>,
    ) -> Result<VariableInstance, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let (basis, committed) = {
            let publication = self.mutation_publication.lock().unwrap();
            let basis = self.capture_variable_staging_basis(&publication)?;
            let data = self.project_data.read().unwrap();
            let existing = data
                .variables
                .values()
                .map(|variable| variable.name.as_str())
                .collect::<Vec<_>>();
            let variable = VariableInstance {
                id: VariableId::new(),
                name: unique_name::unique_name(name, existing),
                data_type,
                data_value,
                tabular: None,
                description: description.to_string(),
                scope,
                tags,
            };
            drop(data);
            drop(publication);
            self.run_variable_staging_test_hook();
            let variable = Self::stage_variable(variable)?;
            (basis, variable)
        };

        let mut publication = self.mutation_publication.lock().unwrap();
        self.validate_variable_staging_basis(&publication, &basis)?;
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        self.ensure_project_operational()?;
        let id = committed.id;
        data.variables.insert(id, committed.clone());
        revisions.insert(
            id,
            crate::project::project_state::VariableRevisionEntry::present(
                crate::project::ResourceRevision::INITIAL,
            ),
        );
        publication.advance_authority_generation();
        Ok(committed)
    }

    #[cfg(test)]
    pub(crate) fn remove_variable(
        &self,
        variable_id: &VariableId,
    ) -> Result<Option<VariableInstance>, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        self.ensure_project_operational()?;
        let removed = data.variables.remove(variable_id);
        if removed.is_some() {
            let revision = revisions
                .get(variable_id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::project::ResourceRevision::INITIAL)
                .next();
            revisions.insert(
                *variable_id,
                crate::project::project_state::VariableRevisionEntry::deleted(revision),
            );
            publication.advance_authority_generation();
        }
        Ok(removed)
    }

    pub fn get_variable(
        &self,
        variable_id: &VariableId,
    ) -> Result<Option<VariableInstance>, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self
            .project_data
            .read()
            .unwrap()
            .variables
            .get(variable_id)
            .cloned())
    }

    /// 更新变量（部分字段），返回更新后的实例
    #[cfg(test)]
    pub(crate) fn update_variable(
        &self,
        variable_id: &VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<Option<VariableInstance>, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let (basis, mut variable) = {
            let publication = self.mutation_publication.lock().unwrap();
            let basis = self.capture_variable_staging_basis(&publication)?;
            let variable = self
                .project_data
                .read()
                .unwrap()
                .variables
                .get(variable_id)
                .cloned();
            let Some(variable) = variable else {
                return Ok(None);
            };
            (basis, variable)
        };
        self.run_variable_staging_test_hook();
        if let Some(n) = name {
            variable.name = n;
        }
        if let Some(dt) = data_type {
            let changed = variable.data_type != dt;
            variable.data_type = dt;
            if changed && data_value.is_none() {
                variable.data_value =
                    crate::project::variable_defaults::default_value_for(&variable.data_type);
            }
        }
        if let Some(dv) = data_value {
            variable.data_value = dv;
        }
        if let Some(d) = description {
            variable.description = d;
        }
        if let Some(t) = tags {
            variable.tags = t;
        }
        let updated = Self::stage_variable(variable)?;

        let mut publication = self.mutation_publication.lock().unwrap();
        self.validate_variable_staging_basis(&publication, &basis)?;
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        self.ensure_project_operational()?;
        if !data.variables.contains_key(variable_id) {
            return Ok(None);
        }
        data.variables.insert(*variable_id, updated.clone());
        let revision = revisions
            .get(variable_id)
            .map(|entry| entry.revision)
            .unwrap_or(crate::project::ResourceRevision::INITIAL)
            .next();
        revisions.insert(
            *variable_id,
            crate::project::project_state::VariableRevisionEntry::present(revision),
        );
        publication.advance_authority_generation();
        Ok(Some(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectData;
    use crate::project::ResourceRevision;
    use std::sync::{Arc, Mutex, mpsc};
    use yss_data_contract::DataSeriesValue;

    fn add_int_variable(state: &ProjectState) -> VariableInstance {
        state
            .add_variable(
                "x",
                DataType::Int64,
                DataValue::Int64(42),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap()
    }

    fn authority_snapshot(
        state: &ProjectState,
    ) -> (
        serde_json::Value,
        std::collections::HashMap<VariableId, crate::project::project_state::VariableRevisionEntry>,
        u64,
    ) {
        (
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            state.variable_revisions.read().unwrap().clone(),
            state.authority_generation_for_test(),
        )
    }

    fn tabular_variable(id: VariableId, name: &str, values: &str) -> VariableInstance {
        VariableInstance {
            id,
            name: name.into(),
            data_type: DataType::DataFrame,
            data_value: DataValue::DataFrame(format!(r#"{{"value":{values}}}"#)),
            tabular: None,
            description: format!("{name} description"),
            scope: VariableScope::Global,
            tags: vec![name.into()],
        }
    }

    fn project_with_variable(variable: VariableInstance) -> ProjectData {
        let mut project = ProjectData::new();
        project.variables.insert(variable.id, variable);
        project
    }

    fn install_staging_barrier(state: &ProjectState) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (captured_tx, captured_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let resume_rx = Mutex::new(resume_rx);
        state.set_variable_staging_test_hook(Arc::new(move || {
            captured_tx.send(()).unwrap();
            resume_rx.lock().unwrap().recv().unwrap();
        }));
        (captured_rx, resume_tx)
    }

    fn same_root(label: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "yssbi-variable-staging-{label}-{}",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned()
    }

    fn active_state(label: &str) -> ProjectState {
        let state = ProjectState::new();
        state.activate_project_fixture(same_root(label), ProjectData::new());
        state
    }

    #[test]
    fn remove_variable_retains_next_revision_tombstone() {
        let state = active_state("remove-tombstone");
        let variable = add_int_variable(&state);
        assert_eq!(
            state.revision_state_for_test().1.get(&variable.id),
            Some(&ResourceRevision::INITIAL)
        );

        state.remove_variable(&variable.id).unwrap();

        assert!(state.get_variable(&variable.id).unwrap().is_none());
        assert_eq!(
            state.revision_state_for_test().1.get(&variable.id),
            Some(&ResourceRevision::new(1))
        );
        assert!(
            !state
                .variable_revision_entry_for_test(&variable.id)
                .unwrap()
                .is_present()
        );
    }

    #[test]
    fn stale_add_variable_staging_rejects_same_generation_reactivated_project() {
        let state = ProjectState::new();
        let root = same_root("add");
        state.activate_project_fixture(root.clone(), ProjectData::new());
        assert_eq!(state.authority_generation_for_test(), 0);
        let (captured_rx, resume_tx) = install_staging_barrier(&state);
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            worker_state.add_variable(
                "from project a",
                DataType::Int64,
                DataValue::Int64(1),
                "must not enter project b",
                VariableScope::Global,
                vec!["project-a".into()],
            )
        });
        captured_rx.recv().unwrap();

        let b_variable = tabular_variable(VariableId::new(), "project b", "[20,21]");
        state.activate_project_fixture(root, project_with_variable(b_variable));
        assert_eq!(state.authority_generation_for_test(), 0);
        let before = authority_snapshot(&state);
        resume_tx.send(()).unwrap();

        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(authority_snapshot(&state), before);
    }

    #[test]
    fn stale_update_variable_staging_rejects_same_uuid_in_reactivated_project() {
        let state = ProjectState::new();
        let root = same_root("update");
        let shared_id = VariableId::new();
        state.activate_project_fixture(
            root.clone(),
            project_with_variable(tabular_variable(shared_id, "project a", "[1,2]")),
        );
        assert_eq!(state.authority_generation_for_test(), 0);
        let (captured_rx, resume_tx) = install_staging_barrier(&state);
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            worker_state.update_variable(
                &shared_id,
                Some("stale project a update".into()),
                None,
                None,
                Some("must not enter project b".into()),
                Some(vec!["project-a".into()]),
            )
        });
        captured_rx.recv().unwrap();

        state.activate_project_fixture(
            root,
            project_with_variable(tabular_variable(shared_id, "project b", "[30,31,32]")),
        );
        assert_eq!(state.authority_generation_for_test(), 0);
        let before = authority_snapshot(&state);
        resume_tx.send(()).unwrap();

        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(authority_snapshot(&state), before);
    }

    #[test]
    fn failed_add_variable_has_zero_authority_effects() {
        let state = active_state("failed-add");
        let before = authority_snapshot(&state);

        let result = state.add_variable(
            "invalid series",
            DataType::DataSeries(Box::new(DataType::Int64)),
            DataValue::DataSeries(DataSeriesValue::new(r#"{"a":[1],"b":[2]}"#)),
            "",
            VariableScope::Global,
            vec![],
        );

        assert!(result.is_err());
        assert_eq!(authority_snapshot(&state), before);
    }

    #[test]
    fn failed_update_variable_has_zero_authority_effects() {
        let state = active_state("failed-update");
        let variable = add_int_variable(&state);
        let before = authority_snapshot(&state);

        let result = state.update_variable(
            &variable.id,
            Some("must not commit".into()),
            Some(DataType::DataSeries(Box::new(DataType::Int64))),
            Some(DataValue::DataSeries(DataSeriesValue::new(
                r#"{"a":[1],"b":[2]}"#,
            ))),
            Some("must not commit".into()),
            Some(vec!["must-not-commit".into()]),
        );

        assert!(result.is_err());
        assert_eq!(authority_snapshot(&state), before);
    }

    #[test]
    fn successful_variable_mutations_advance_generation_exactly_once() {
        let state = active_state("successful-generation");
        let initial = state.authority_generation_for_test();

        let variable = add_int_variable(&state);
        assert_eq!(state.authority_generation_for_test(), initial + 1);

        state
            .update_variable(&variable.id, Some("updated".into()), None, None, None, None)
            .unwrap();
        assert_eq!(state.authority_generation_for_test(), initial + 2);
    }

    #[test]
    fn update_variable_resets_value_to_type_default_when_type_changes_without_value() {
        let state = active_state("default-value");
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                None,
                None,
                None,
            )
            .expect("variable update succeeds")
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(false));
    }

    #[test]
    fn update_variable_resets_to_default_array_when_type_changes() {
        let state = active_state("default-array");
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Array(Box::new(DataType::Any))),
                None,
                None,
                None,
            )
            .expect("variable update succeeds")
            .expect("updated variable");

        assert_eq!(
            updated.data_value,
            DataValue::Array(vec![
                DataValue::Int64(1),
                DataValue::Int64(2),
                DataValue::Int64(3),
            ])
        );
    }

    #[test]
    fn update_variable_resets_to_default_object_when_type_changes() {
        let state = active_state("default-object");
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(&variable.id, None, Some(DataType::Object), None, None, None)
            .expect("variable update succeeds")
            .expect("updated variable");

        let DataValue::Object(map) = updated.data_value else {
            panic!("expected object value");
        };
        assert_eq!(map.get("key_0"), Some(&DataValue::Int64(1)));
        assert_eq!(map.get("key_1"), Some(&DataValue::Int64(2)));
    }

    #[test]
    fn update_variable_keeps_explicit_value_when_type_and_value_are_both_changed() {
        let state = active_state("explicit-value");
        let variable = add_int_variable(&state);

        let updated = state
            .update_variable(
                &variable.id,
                None,
                Some(DataType::Boolean),
                Some(DataValue::Boolean(true)),
                None,
                None,
            )
            .expect("variable update succeeds")
            .expect("updated variable");

        assert_eq!(updated.data_type, DataType::Boolean);
        assert_eq!(updated.data_value, DataValue::Boolean(true));
    }
}
