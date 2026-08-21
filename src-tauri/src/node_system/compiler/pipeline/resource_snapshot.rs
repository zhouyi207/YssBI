use super::*;

pub trait ResourceSnapshot {
    fn versions(&self) -> ResourceVersionSet;

    fn version(&self, key: &ResourceKey) -> Option<ResourceVersion> {
        self.versions().remove(key)
    }

    fn observed_state(&self, key: &ResourceKey) -> ResourceObservedState {
        self.version(key)
            .map(ResourceObservedState::Present)
            .unwrap_or(ResourceObservedState::Absent(None))
    }

    fn function_name(&self, _path: &GraphResourcePath) -> Option<&str> {
        None
    }

    fn function_document(&self, _path: &GraphResourcePath) -> Option<&FunctionDocument> {
        None
    }

    fn function_graph_document(&self, _path: &GraphResourcePath) -> Option<&GraphDocument> {
        None
    }

    fn variable(
        &self,
        _id: &crate::variable::VariableId,
    ) -> Option<&crate::variable::VariableInstance> {
        None
    }

    fn database_name(&self, _id: &str) -> Option<&str> {
        None
    }

    fn database_schema(&self, _id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        None
    }
}

pub(super) struct TrackedResourceResolver<'a, S> {
    snapshot: &'a S,
    reads: AnalysisResourceReads,
    observations: ResourceObservationSet,
}

impl<'a, S> TrackedResourceResolver<'a, S> {
    pub(super) fn new(snapshot: &'a S) -> Self {
        Self {
            snapshot,
            reads: AnalysisResourceReads::new(),
            observations: ResourceObservationSet::new(),
        }
    }
}

impl<S: ResourceSnapshot> TrackedResourceResolver<'_, S> {
    fn failure(
        &mut self,
        key: ResourceKey,
        state: ResourceObservedState,
        reason: impl Into<Box<str>>,
    ) -> ResourceResolutionError {
        let reason = reason.into();
        self.observations.insert(key.clone(), state.clone());
        ResourceResolutionError::new(key, state, reason)
    }

    fn successful(&mut self, key: ResourceKey, version: ResourceVersion) {
        self.observations.remove(&key);
        self.reads.insert(key, version);
    }
}

impl<S: ResourceSnapshot> AnalysisResourceResolver for TrackedResourceResolver<'_, S> {
    fn resolve_function(
        &mut self,
        path: &GraphResourcePath,
    ) -> Result<ResolvedFunction<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(path.0.clone());
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(
                key,
                state,
                format!("function resource '{}' is missing", path.0),
            ));
        };
        let name = self.snapshot.function_name(path);
        let Some(function) = self.snapshot.function_document(path) else {
            return Err(self.failure(
                key,
                state,
                format!("function resource '{}' has no signature", path.0),
            ));
        };
        let Some(graph) = self.snapshot.function_graph_document(path) else {
            return Err(self.failure(
                key,
                state,
                format!("function graph '{}' is missing", path.0),
            ));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value: ResolvedFunctionValue {
                name,
                function,
                graph,
            },
        })
    }

    fn resolve_variable(
        &mut self,
        id: &crate::variable::VariableId,
    ) -> Result<ResolvedVariable<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(format!("variables/{id}"));
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(key, state, format!("variable resource '{id}' is missing")));
        };
        let Some(value) = self.snapshot.variable(id) else {
            return Err(self.failure(key, state, format!("variable resource '{id}' has no value")));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value,
        })
    }

    fn resolve_database(
        &mut self,
        id: &str,
    ) -> Result<ResolvedDatabase<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(format!("databases/{id}"));
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(key, state, format!("database resource '{id}' is missing")));
        };
        let name = self.snapshot.database_name(id);
        let Some(columns) = self.snapshot.database_schema(id) else {
            return Err(self.failure(
                key,
                state,
                format!("database resource '{id}' has no schema"),
            ));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value: ResolvedDatabaseValue { name, columns },
        })
    }

    fn reads(&self) -> &AnalysisResourceReads {
        &self.reads
    }

    fn observations(&self) -> &ResourceObservationSet {
        &self.observations
    }
}
