use super::*;

struct CommittedGraphMutation {
    project_instance_id: String,
    delta: GraphDeltaEvent<GraphDocumentPatch>,
    projection_replacement: crate::event::GraphProjectionReplacementDto,
    history: HistoryStatusDto,
}

impl ProjectState {
    pub fn apply_editor_graph_mutation(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.apply_editor_graph_mutation_observed(
            project_instance_id,
            graph_path,
            locale,
            request,
            |_| {},
        )
    }

    pub fn export_editor_subgraph(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        node_ids: Vec<NodeId>,
    ) -> Result<ClipboardSubgraphDto, MutationConflict> {
        self.ensure_mutation_operational()?;
        let catalog = self
            .catalog_mutation_validation_snapshot(project_instance_id)
            .map_err(|error| match error {
                ProjectFilesystemError::StaleProjectLifecycle { message } => {
                    MutationConflict::StaleProjectLifecycle(message.into())
                }
                ProjectFilesystemError::CatalogResourceStale { message } => {
                    MutationConflict::CatalogResourceStale(message.into())
                }
                error => MutationConflict::CatalogResourceStale(error.to_string().into()),
            })?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        let (document, registry) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != project_instance_id.as_str() {
                return Err(MutationConflict::StaleProjectLifecycle(
                    "project changed before subgraph export".into(),
                ));
            }
            if publication.authority_generation() != catalog.authority_generation {
                return Err(MutationConflict::CatalogResourceStale(
                    "catalog or graph authority changed before subgraph export".into(),
                ));
            }
            let data = self.project_data.read().unwrap();
            let document = data
                .graphs
                .get(graph_path)
                .map(|graph| graph.document.clone())
                .ok_or_else(|| MutationConflict::ResourceMismatch {
                    requested: expected_resource.clone(),
                    store: expected_resource,
                })?;
            let registry = Arc::clone(&self.project_store.read().unwrap().node_registry);
            (document, registry)
        };

        export_subgraph(&node_path, &document, registry.as_ref(), &catalog, node_ids)
    }

    pub fn apply_editor_graph_mutation_observed(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
        observe: impl FnOnce(&GraphDeltaEvent<GraphDocumentPatch>),
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.apply_editor_graph_mutation_observed_with_allocator(
            project_instance_id,
            graph_path,
            locale,
            request,
            observe,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn apply_editor_graph_mutation_with_allocator_for_test(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
        allocate_connection_id: &(dyn Fn() -> ConnectionId + Send + Sync),
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.apply_editor_graph_mutation_observed_with_allocator(
            project_instance_id,
            graph_path,
            locale,
            request,
            |_| {},
            Some(allocate_connection_id),
        )
    }

    fn apply_editor_graph_mutation_observed_with_allocator(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
        observe: impl FnOnce(&GraphDeltaEvent<GraphDocumentPatch>),
        #[cfg(test)] allocate_connection_id: Option<&(dyn Fn() -> ConnectionId + Send + Sync)>,
    ) -> Result<GraphMutationResultDto, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let connect_from = match &request.payload {
            EditorGraphMutationDto::CreateNode { connect_from, .. } => connect_from.clone(),
            _ => None,
        };
        let map_catalog_error = |error: ProjectFilesystemError| match error {
            ProjectFilesystemError::StaleProjectLifecycle { message } => {
                MutationConflict::StaleProjectLifecycle(message.into())
            }
            ProjectFilesystemError::CatalogResourceStale { message } => {
                MutationConflict::CatalogResourceStale(message.into())
            }
            error => MutationConflict::CatalogResourceStale(error.to_string().into()),
        };
        let compatibility_catalog = connect_from
            .as_ref()
            .map(|_| {
                self.catalog_snapshot(project_instance_id)
                    .map_err(map_catalog_error)
            })
            .transpose()?;
        let catalog_snapshot = if let Some(snapshot) = compatibility_catalog.as_ref() {
            Some(snapshot.validation.clone())
        } else {
            match &request.payload {
                EditorGraphMutationDto::CreateNode {
                    descriptor:
                        crate::node_system::catalog::NodeCreationDescriptor::ResourceBound { .. },
                    ..
                }
                | EditorGraphMutationDto::DuplicateSubgraph { .. }
                | EditorGraphMutationDto::InsertSubgraph { .. } => Some(
                    self.catalog_mutation_validation_snapshot(project_instance_id)
                        .map_err(map_catalog_error)?,
                ),
                _ => None,
            }
        };
        let compatibility_source = if let (Some(source_port), Some(snapshot)) =
            (connect_from, compatibility_catalog.as_ref())
        {
            let projection = self
                .graph_projection_for_project(project_instance_id, graph_path, locale)
                .map_err(|error| MutationConflict::Projection(error.to_string().into()))?;
            if projection.basis.graph_revision != request.base_revision.get() {
                return Err(MutationConflict::StaleRevision {
                    base_revision: request.base_revision,
                    current_revision: ResourceRevision::new(projection.basis.graph_revision),
                });
            }
            let mut source = crate::node_system::compatibility::source_from_projection(
                &projection,
                snapshot.registry.as_ref(),
                source_port,
            )
            .map_err(|message| MutationConflict::InvalidEditorMutation(message.into()))?;
            let document = self
                .get_data()
                .map_err(|error| MutationConflict::Projection(error.to_string().into()))?
                .graphs
                .get(graph_path)
                .map(|resource| resource.document.clone())
                .ok_or_else(|| MutationConflict::Projection("graph is not loaded".into()))?;
            crate::node_system::compatibility::refine_source_type(
                &mut source,
                &document,
                snapshot.registry.as_ref(),
                &snapshot.validation,
            );
            self.validate_catalog_snapshot_current(snapshot)
                .map_err(map_catalog_error)?;
            Some(source)
        } else {
            None
        };
        let mutation_validation = if matches!(
            &request.payload,
            EditorGraphMutationDto::Connect { .. }
                | EditorGraphMutationDto::MoveConnections { .. }
                | EditorGraphMutationDto::CreateNode {
                    connect_from: Some(_),
                    ..
                }
        ) {
            let expected_session = self.current_projection_environment_expectation();
            let environment = self
                .capture_projection_environment(&expected_session)
                .map_err(|error| MutationConflict::Projection(error.into()))?;
            let data = self
                .get_data()
                .map_err(|error| MutationConflict::Projection(error.to_string().into()))?;
            let projection_source = self.projection_source_snapshot(
                &data,
                environment.clone(),
                environment.authority.project_instance_id.clone(),
                environment.authority.authority_generation,
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.database_authority_revisions.read().unwrap().clone(),
            );
            let projection = projection_source
                .graph_projection(graph_path, locale)
                .map_err(|error| MutationConflict::Projection(error.into()))?;
            let snapshot = crate::node_system::compatibility::EditorMutationValidationSnapshot::from_projection(
                &projection,
                environment.registry.as_ref(),
            )
            .map_err(|error| MutationConflict::Projection(error.into()))?;
            if snapshot.graph_revision != request.base_revision {
                return Err(MutationConflict::StaleRevision {
                    base_revision: request.base_revision,
                    current_revision: snapshot.graph_revision,
                });
            }
            Some((snapshot, environment))
        } else {
            None
        };
        let (projected_connect, projected_connect_authority_generation) =
            if let EditorGraphMutationDto::Connect { output, input, .. } = &request.payload {
                let output = crate::node_system::document::PortAddress::try_from(output.clone())
                    .map_err(|message: String| {
                        MutationConflict::InvalidEditorMutation(message.into())
                    })?;
                let input = crate::node_system::document::PortAddress::try_from(input.clone())
                    .map_err(|message: String| {
                        MutationConflict::InvalidEditorMutation(message.into())
                    })?;
                let source = self
                    .capture_projection_source(graph_path)
                    .map_err(|error| MutationConflict::Projection(error.into()))?;
                if source.project_instance_id != project_instance_id.as_str() {
                    return Err(MutationConflict::StaleProjectLifecycle(
                        "project changed before projected connection resolution".into(),
                    ));
                }
                let (analysis, _) = self
                    .get_or_compile_current_from_source(graph_path, &source)
                    .map_err(|error| MutationConflict::Projection(error.into()))?;
                let projection = &analysis.payload.interface_projection;
                if projection.basis.graph_revision != request.base_revision {
                    return Err(MutationConflict::CompilationBasisStale {
                        basis_revision: projection.basis.graph_revision,
                        current_revision: request.base_revision,
                    });
                }
                let candidates: Vec<_> = [output.clone(), input.clone()]
                    .into_iter()
                    .filter_map(|address| {
                        projection
                            .authorize_materialization_candidate(&node_path, &address)
                            .map(|(candidate, member, authorization)| {
                                (address, candidate, member, authorization)
                            })
                    })
                    .collect();
                if candidates.len() > 1 {
                    return Err(MutationConflict::InvalidEditorMutation(
                        "connections between two projected members are not supported".into(),
                    ));
                }
                if candidates.is_empty() {
                    let document = &source
                        .data
                        .graphs
                        .get(graph_path)
                        .expect("captured projection source contains the target graph")
                        .document;
                    let has_unbound_instance = [&output, &input].into_iter().any(|address| {
                        address.is_instance() && !document.port_bindings.contains_key(address)
                    });
                    if has_unbound_instance {
                        return Err(MutationConflict::InvalidEditorMutation(
                            "projected connection endpoint is stale or unavailable".into(),
                        ));
                    }
                }
                let plan = candidates.into_iter().next().map(
                    |(address, candidate, member, authorization)| {
                        crate::node_system::document::ProjectedConnectPlan {
                            projection_address: address,
                            direction: candidate.direction(),
                            kind: candidate.kind(),
                            connections: candidate.connections(),
                            member,
                            authorization,
                        }
                    },
                );
                let authority_generation = plan.as_ref().map(|_| source.authority_generation);
                (plan, authority_generation)
            } else {
                (None, None)
            };
        if catalog_snapshot.is_some() {
            self.run_catalog_mutation_before_publication_test_hook();
        }
        let committed = self.commit_editor_graph_mutation(
            project_instance_id,
            graph_path,
            locale,
            request,
            catalog_snapshot.as_ref(),
            compatibility_source.as_ref(),
            mutation_validation.as_ref(),
            projected_connect,
            projected_connect_authority_generation,
            #[cfg(test)]
            allocate_connection_id,
        )?;
        if !committed.delta.payload.operations.is_empty() {
            observe(&committed.delta);
        }
        Ok(GraphMutationResultDto {
            project_instance_id: committed.project_instance_id,
            delta: committed.delta,
            projection_replacement: committed.projection_replacement,
            history: committed.history,
        })
    }

    #[cfg(test)]
    pub(crate) fn apply_graph_mutation(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphMutation>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        self.ensure_mutation_operational()?;
        let resource = data.graphs.get(graph_path).cloned().ok_or_else(|| {
            MutationConflict::ResourceMismatch {
                requested: request.resource.clone(),
                store: ResourceKey::Graph(node_path.clone()),
            }
        })?;
        let mut planner = RevisionedGraphStore::new(node_path.clone(), resource.document.clone());
        let event = planner.apply_mutation(request)?;
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction = ProjectHistoryTransaction::graph(
            event
                .caused_by
                .expect("mutation events carry operation IDs"),
            node_path,
            event.from_revision,
            event.payload.clone(),
        );
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        for (path, graph) in &mut data.graphs {
            let key = crate::node_system::document::GraphResourcePath(path.as_str().into());
            if let Some(document) = documents.graphs.remove(&key) {
                graph.document = document;
            }
        }
        let revision = data
            .graphs
            .get(graph_path)
            .expect("mutated graph remains loaded")
            .document
            .revision;
        self.graph_revisions
            .write()
            .unwrap()
            .insert(graph_path.clone(), revision);
        publication.advance_authority_generation();
        self.invalidate_graph_compile_products(graph_path);
        Ok(event)
    }

    #[cfg(test)]
    pub(crate) fn apply_graph_patch(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphDocumentPatch>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        self.commit_graph_patch(graph_path, request)
            .map(|committed| committed.delta)
    }

    #[cfg(test)]
    fn commit_graph_patch(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphDocumentPatch>,
    ) -> Result<CommittedGraphMutation, MutationConflict> {
        let MutationRequest {
            resource,
            base_revision,
            operation_id,
            payload,
        } = request;
        self.commit_graph_patch_planned(
            graph_path,
            resource,
            base_revision,
            operation_id,
            "en-US",
            None,
            None,
            None,
            None,
            move |_, _| Ok(payload),
        )
    }

    fn commit_editor_graph_mutation(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<EditorGraphMutationDto>,
        catalog_snapshot: Option<&crate::project::CatalogMutationValidationSnapshot>,
        compatibility_source: Option<&crate::node_system::compatibility::SourcePort>,
        mutation_validation: Option<&(
            crate::node_system::compatibility::EditorMutationValidationSnapshot,
            ProjectionEnvironmentSnapshot,
        )>,
        projected_connect: Option<crate::node_system::document::ProjectedConnectPlan>,
        projected_connect_authority_generation: Option<u64>,
        #[cfg(test)] allocate_connection_id: Option<&(dyn Fn() -> ConnectionId + Send + Sync)>,
    ) -> Result<CommittedGraphMutation, MutationConflict> {
        let MutationRequest {
            resource,
            base_revision,
            operation_id,
            payload,
        } = request;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        self.commit_graph_patch_planned(
            graph_path,
            resource,
            base_revision,
            operation_id,
            locale,
            Some(project_instance_id),
            catalog_snapshot,
            mutation_validation.map(|(_, environment)| environment.clone()),
            projected_connect_authority_generation,
            move |document, registry| {
                #[cfg(test)]
                if let Some(allocate_connection_id) = allocate_connection_id {
                    return payload.into_patch_with_editor_validation_and_allocator(
                        &node_path,
                        document,
                        registry,
                        catalog_snapshot,
                        compatibility_source,
                        mutation_validation.map(|(snapshot, _)| snapshot),
                        projected_connect,
                        allocate_connection_id,
                    );
                }
                payload.into_patch_with_editor_validation_and_projected_connect(
                    &node_path,
                    document,
                    registry,
                    catalog_snapshot,
                    compatibility_source,
                    mutation_validation.map(|(snapshot, _)| snapshot),
                    projected_connect,
                )
            },
        )
    }

    fn commit_graph_patch_planned(
        &self,
        graph_path: &GraphResourcePath,
        resource: ResourceKey,
        base_revision: ResourceRevision,
        operation_id: OperationId,
        locale: &str,
        project_instance_id: Option<&ProjectInstanceId>,
        catalog_snapshot: Option<&crate::project::CatalogMutationValidationSnapshot>,
        projection_environment: Option<ProjectionEnvironmentSnapshot>,
        expected_authority_generation: Option<u64>,
        plan: impl FnOnce(
            &crate::node_system::document::GraphDocument,
            &crate::node_system::registry::NodeRegistry,
        ) -> Result<GraphDocumentPatch, MutationConflict>,
    ) -> Result<CommittedGraphMutation, MutationConflict> {
        self.ensure_mutation_operational()?;
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        if resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: resource,
                store: expected_resource,
            });
        }
        let expected_session = self.current_projection_environment_expectation();
        let projection_environment = match projection_environment {
            Some(environment) => environment,
            None => self
                .capture_projection_environment(&expected_session)
                .map_err(|error| MutationConflict::Projection(error.into()))?,
        };
        let (
            captured_data,
            captured_graph_revisions,
            captured_variable_revisions,
            captured_database_revisions,
        ) = {
            let data = self.project_data.read().unwrap();
            let graph =
                data.graphs
                    .get(graph_path)
                    .ok_or_else(|| MutationConflict::ResourceMismatch {
                        requested: expected_resource.clone(),
                        store: expected_resource.clone(),
                    })?;
            if graph.document.revision != base_revision {
                return Err(MutationConflict::StaleRevision {
                    base_revision,
                    current_revision: graph.document.revision,
                });
            }
            (
                data.clone(),
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.database_authority_revisions.read().unwrap().clone(),
            )
        };
        let captured_document = &captured_data.graphs[graph_path].document;
        let patch = plan(captured_document, projection_environment.registry.as_ref())?;
        let mut candidate_data = captured_data;
        let candidate_graph = candidate_data
            .graphs
            .get_mut(graph_path)
            .expect("captured graph remains present");
        candidate_graph.document.apply_patch(&patch)?;
        let candidate_revision = candidate_graph.document.revision;
        let mut candidate_graph_revisions = captured_graph_revisions;
        candidate_graph_revisions.insert(graph_path.clone(), candidate_revision);
        let candidate_source = self.projection_source_snapshot(
            &candidate_data,
            projection_environment.clone(),
            projection_environment
                .authority
                .project_instance_id
                .as_str()
                .to_owned(),
            projection_environment
                .authority
                .authority_generation
                .saturating_add(u64::from(!patch.operations.is_empty())),
            candidate_graph_revisions,
            captured_variable_revisions,
            captured_database_revisions,
        );
        let projection_replacement =
            candidate_projection_replacement(&candidate_source, graph_path, locale)?;
        self.run_mutation_publication_test_hook();
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        if project_instance_id
            .is_some_and(|expected| publication.project_instance_id != expected.as_str())
        {
            return Err(MutationConflict::StaleProjectLifecycle(
                "caller project changed before graph authority commit".into(),
            ));
        }
        if expected_authority_generation
            .is_some_and(|expected| publication.authority_generation() != expected)
        {
            return Err(MutationConflict::CompilationBasisStale {
                basis_revision: base_revision,
                current_revision: data
                    .graphs
                    .get(graph_path)
                    .map(|graph| graph.document.revision)
                    .unwrap_or(base_revision),
            });
        }
        if let Some(snapshot) = catalog_snapshot {
            if publication.project_instance_id != snapshot.project_instance_id.as_str()
                || publication.authority_generation() != snapshot.authority_generation
            {
                return Err(MutationConflict::CatalogResourceStale(
                    "catalog authority changed before graph mutation publication".into(),
                ));
            }
        }
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(MutationConflict::StaleProjectLifecycle(
                "project changed before graph authority commit".into(),
            ));
        }
        self.ensure_mutation_operational()?;
        let graph =
            data.graphs
                .get(graph_path)
                .ok_or_else(|| MutationConflict::ResourceMismatch {
                    requested: expected_resource.clone(),
                    store: expected_resource.clone(),
                })?;
        if graph.document.revision != base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision,
                current_revision: graph.document.revision,
            });
        }
        if !projection_environment.matches_publication(&publication) {
            return Err(MutationConflict::StaleProjectLifecycle(
                "projection environment changed before graph authority commit".into(),
            ));
        }
        if patch.operations.is_empty() {
            let history = self.history.read().unwrap().status();
            return Ok(CommittedGraphMutation {
                project_instance_id: publication.project_instance_id.clone(),
                delta: GraphDeltaEvent {
                    graph_path: node_path,
                    from_revision: base_revision,
                    to_revision: base_revision,
                    caused_by: Some(operation_id),
                    payload: patch,
                },
                projection_replacement,
                history,
            });
        }
        let publication_advance = publication
            .prepare_authority_generation()
            .map_err(|error| MutationConflict::Projection(error.to_string().into()))?;
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction =
            ProjectHistoryTransaction::graph(operation_id, node_path, base_revision, patch.clone());
        let mut history = self.history.write().unwrap();
        history
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;

        let updated = documents
            .graphs
            .remove(&crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ))
            .expect("patched graph remains present");
        let to_revision = updated.revision;
        data.graphs
            .get_mut(graph_path)
            .expect("graph remains loaded")
            .document = updated;
        graph_revisions.insert(graph_path.clone(), to_revision);
        let history = history.status();
        publication.commit_prepared(publication_advance);
        self.invalidate_graph_compile_products(graph_path);
        Ok(CommittedGraphMutation {
            project_instance_id: publication.project_instance_id.clone(),
            delta: GraphDeltaEvent {
                graph_path: crate::node_system::document::GraphResourcePath(
                    graph_path.as_str().into(),
                ),
                from_revision: base_revision,
                to_revision,
                caused_by: Some(operation_id),
                payload: patch,
            },
            projection_replacement,
            history,
        })
    }
}
