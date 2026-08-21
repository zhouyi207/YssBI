use super::*;

#[test]
fn phase2_reroute_protocol_validator_rejects_each_malformed_contract() {
    use crate::node_system::catalog::{
        REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT, validate_reroute_protocol_contract,
    };
    use crate::node_system::protocol::{
        ConnectionsPerPort, EffectSemantics, LiteralPolicy, PortDirection, PortKind, Purity,
        SchemaExpr, TypeExpr,
    };
    use crate::node_system::registry::{RegisteredNode, TransparentNodeRole};
    use std::sync::Arc;

    let builtin = build_builtin_node_system().unwrap();
    for kind in [PortKind::Data, PortKind::Control, PortKind::Effect] {
        let node_type = crate::node_system::catalog::reroute_node_type_for_kind(kind);
        let registered = builtin.registry.get(&node_type).unwrap();
        assert!(validate_reroute_protocol_contract(registered, kind).is_ok());

        let malformed = |mutate: fn(&mut crate::node_system::protocol::NodeProtocol)| {
            let mut protocol = registered.protocol().clone();
            mutate(&mut protocol);
            RegisteredNode::transparent(Arc::new(protocol), TransparentNodeRole::Reroute)
        };
        let mut cases = vec![
            malformed(|protocol| protocol.catalog.hidden = false),
            malformed(|protocol| {
                protocol.catalog.style_id =
                    crate::node_system::protocol::NodeStyleId::new("wrong").unwrap()
            }),
            malformed(|protocol| {
                protocol.parameters.parameters = vec![crate::node_system::protocol::ParameterSpec {
                    key: crate::node_system::protocol::ParameterKey::new("wrong").unwrap(),
                    title_key: crate::node_system::protocol::I18nKey::new("nodes.wrong").unwrap(),
                    description_key: None,
                    value_type: TypeExpr::Unknown,
                    default_value: None,
                    constraints: Vec::new(),
                    editor: crate::node_system::protocol::ParameterEditorSpec::Auto,
                    presentation: crate::node_system::protocol::ParameterPresentation::DetailPanel,
                }]
                .into_boxed_slice()
            }),
            malformed(|protocol| {
                protocol.interface.member_groups =
                    vec![crate::node_system::protocol::PortMemberGroupSpec {
                        templates: vec![
                            crate::node_system::protocol::PortKey::new(REROUTE_INPUT_PORT).unwrap(),
                        ]
                        .into_boxed_slice(),
                        min: 0,
                        max: None,
                    }]
                    .into_boxed_slice()
            }),
            malformed(|protocol| {
                protocol.managed_role =
                    Some(crate::node_system::protocol::ManagedNodeRole::EventBegin)
            }),
            malformed(|protocol| protocol.interface.ports.swap(0, 1)),
            malformed(|protocol| protocol.interface.ports[0].direction = PortDirection::Output),
            malformed(|protocol| protocol.interface.ports[1].direction = PortDirection::Input),
            malformed(|protocol| {
                protocol.interface.ports[0].connections = ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                }
            }),
            malformed(|protocol| {
                protocol.interface.ports[1].connections = ConnectionsPerPort::Single
            }),
            malformed(|protocol| {
                protocol.interface.ports[0].label_key =
                    crate::node_system::protocol::I18nKey::new("nodes.wrong.ports.input").unwrap()
            }),
            malformed(|protocol| {
                protocol.interface.ports[0].key =
                    crate::node_system::protocol::PortKey::new("wrong").unwrap()
            }),
            malformed(|protocol| {
                protocol.interface.ports[1].key =
                    crate::node_system::protocol::PortKey::new("wrong").unwrap()
            }),
        ];
        if kind == PortKind::Data {
            cases.extend([
                malformed(|protocol| protocol.interface.type_parameters = Box::new([])),
                malformed(|protocol| {
                    protocol.interface.type_constraints =
                        vec![crate::node_system::protocol::TypeConstraint::Implements(
                            crate::node_system::protocol::TypeTerm::Expr(TypeExpr::Generic(
                                crate::node_system::protocol::TypeParameterId::new("t").unwrap(),
                            )),
                            crate::node_system::protocol::TypeClassId::new("test.wrong").unwrap(),
                        )]
                        .into_boxed_slice()
                }),
                malformed(|protocol| protocol.interface.ports[1].value_type = TypeExpr::Unknown),
                malformed(|protocol| {
                    protocol.interface.ports[0]
                        .input_binding
                        .as_mut()
                        .unwrap()
                        .literal_policy = LiteralPolicy::Allowed
                }),
                malformed(|protocol| protocol.interface.ports[1].schema = None),
            ]);
        } else {
            cases.extend([
                malformed(|protocol| {
                    protocol.interface.ports[0].value_type = TypeExpr::Generic(
                        crate::node_system::protocol::TypeParameterId::new("t").unwrap(),
                    )
                }),
                malformed(|protocol| {
                    protocol.interface.ports[1].schema = Some(SchemaExpr::Input(
                        crate::node_system::protocol::PortKey::new(REROUTE_INPUT_PORT).unwrap(),
                    ))
                }),
                if kind == PortKind::Effect {
                    malformed(|protocol| protocol.execution.purity = Purity::Pure)
                } else {
                    malformed(|protocol| protocol.execution.purity = Purity::Effectful)
                },
                if kind == PortKind::Effect {
                    malformed(|protocol| protocol.execution.effects = EffectSemantics::None)
                } else {
                    malformed(|protocol| protocol.execution.effects = EffectSemantics::Ordered)
                },
            ]);
        }
        for malformed in cases {
            assert!(validate_reroute_protocol_contract(&malformed, kind).is_err());
        }
        assert_eq!(
            registered.protocol().interface.ports[0].key.as_str(),
            REROUTE_INPUT_PORT
        );
        assert_eq!(
            registered.protocol().interface.ports[1].key.as_str(),
            REROUTE_OUTPUT_PORT
        );
    }
}

#[test]
fn phase2_reroute_protocol_contracts_are_exact_for_all_port_kinds() {
    use crate::node_system::catalog::{
        CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
        REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT, reroute_node_type_for_kind,
    };
    use crate::node_system::protocol::{
        EffectSemantics, InputBindingSpec, LiteralPolicy, Purity, SchemaExpr, TypeParameterId,
    };
    use crate::node_system::registry::TransparentNodeRole;

    let builtin = build_builtin_node_system().unwrap();
    let generic = TypeParameterId::new("t").unwrap();
    let cases = [
        (PortKind::Data, DATA_REROUTE_NODE_TYPE),
        (PortKind::Control, CONTROL_REROUTE_NODE_TYPE),
        (PortKind::Effect, EFFECT_REROUTE_NODE_TYPE),
    ];

    for (kind, stable_id) in cases {
        let node_type = reroute_node_type_for_kind(kind);
        assert_eq!(node_type.as_str(), stable_id);
        let registered = builtin.registry.get(&node_type).unwrap();
        let contract =
            crate::node_system::catalog::validate_reroute_protocol_contract(registered, kind)
                .unwrap();
        let protocol = registered.protocol();
        assert_eq!(contract.input_key.as_str(), REROUTE_INPUT_PORT);
        assert_eq!(contract.output_key.as_str(), REROUTE_OUTPUT_PORT);

        assert_eq!(
            registered.transparent_role(),
            Some(TransparentNodeRole::Reroute)
        );
        assert!(registered.implementation().is_none());
        assert!(registered.structural_role().is_none());
        assert!(protocol.catalog.hidden);
        assert_eq!(protocol.catalog.style_id.as_str(), "builtin.reroute");
        assert!(protocol.parameters.parameters.is_empty());
        assert!(protocol.interface.member_groups.is_empty());
        assert_eq!(protocol.managed_role, None);
        assert_eq!(protocol.interface.ports.len(), 2);

        let input = &protocol.interface.ports[0];
        let output = &protocol.interface.ports[1];
        assert_eq!(input.key.as_str(), REROUTE_INPUT_PORT);
        assert_eq!(output.key.as_str(), REROUTE_OUTPUT_PORT);
        assert_eq!(input.direction, PortDirection::Input);
        assert_eq!(output.direction, PortDirection::Output);
        assert_eq!(input.kind, kind);
        assert_eq!(output.kind, kind);
        assert_eq!(input.instances, PortInstances::Declared);
        assert_eq!(output.instances, PortInstances::Declared);
        assert_eq!(input.connections, ConnectionsPerPort::Single);
        assert_eq!(
            output.connections,
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        );

        match kind {
            PortKind::Data => {
                assert_eq!(
                    protocol.interface.type_parameters.as_ref(),
                    &[generic.clone()]
                );
                assert_eq!(input.value_type, TypeExpr::Generic(generic.clone()));
                assert_eq!(output.value_type, TypeExpr::Generic(generic.clone()));
                assert_eq!(
                    input.input_binding,
                    Some(InputBindingSpec {
                        literal_policy: LiteralPolicy::Forbidden,
                        default_value: None,
                    })
                );
                assert_eq!(output.schema, Some(SchemaExpr::Input(input.key.clone())));
                assert_eq!(protocol.execution.purity, Purity::Pure);
                assert_eq!(protocol.execution.effects, EffectSemantics::None);
            }
            PortKind::Control | PortKind::Effect => {
                assert!(protocol.interface.type_parameters.is_empty());
                assert_eq!(input.value_type, TypeExpr::Unknown);
                assert_eq!(output.value_type, TypeExpr::Unknown);
                assert_eq!(input.input_binding, None);
                assert_eq!(input.schema, None);
                assert_eq!(output.schema, None);
                if kind == PortKind::Effect {
                    assert_eq!(protocol.execution.purity, Purity::Effectful);
                    assert_eq!(protocol.execution.effects, EffectSemantics::Ordered);
                } else {
                    assert_eq!(protocol.execution.purity, Purity::Pure);
                    assert_eq!(protocol.execution.effects, EffectSemantics::None);
                }
            }
        }
    }
}

#[test]
fn phase2_reroute_protocol_hidden_nodes_are_absent_from_localized_palette_search() {
    use crate::node_system::catalog::{
        CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
    };

    let builtin = build_builtin_node_system().unwrap();
    let localized = builtin.catalog.localize(&builtin.registry, "en-US");
    let palette_ids = localized
        .items
        .iter()
        .map(|item| item.node_type_id.as_ref())
        .collect::<BTreeSet<_>>();

    for stable_id in [
        DATA_REROUTE_NODE_TYPE,
        CONTROL_REROUTE_NODE_TYPE,
        EFFECT_REROUTE_NODE_TYPE,
    ] {
        assert!(!palette_ids.contains(stable_id));
    }
}
#[test]
fn search_normalization_folds_case_width_diacritics_and_punctuation() {
    assert_eq!(
        normalize_search_text("  ＡＤＤ—Café，SUM_value  ").as_ref(),
        "add cafe sum_value"
    );
    assert_eq!(normalize_search_text("PLUS").as_ref(), "plus");
}

#[test]
fn i18n_validation_uses_default_locale_not_the_locale_union() {
    let catalog = BuiltinCatalog::new(&[
        ("en-US", "required.title", Text("Title")),
        ("en-US", "required.aliases", Aliases(&["one"])),
        ("en-US", "unused.key", Text("Unused")),
        ("zh-CN", "required.aliases", Aliases(&["一"])),
        ("zh-CN", "masked.title", Text("不能掩盖默认缺失")),
    ])
    .unwrap();
    let required = I18nManifest {
        keys: ["required.title", "required.aliases", "masked.title"]
            .into_iter()
            .map(|key| I18nKey::new(key).unwrap())
            .collect(),
    };
    let aliases = BTreeSet::from([I18nKey::new("required.aliases").unwrap()]);

    let inventory = catalog.audit(&required, &aliases);
    assert_eq!(
        inventory.default_locale_missing,
        vec![Box::<str>::from("masked.title")]
    );
    assert_eq!(
        inventory.missing_by_locale["zh-CN"],
        vec![Box::<str>::from("required.title")]
    );
    assert_eq!(
        inventory.unused_by_locale["en-US"],
        vec![Box::<str>::from("unused.key")]
    );
    assert!(matches!(
        catalog.validate(&required, &aliases),
        Err(I18nBundleValidationError::MissingDefaultLocale { .. })
    ));
}

#[test]
fn i18n_validation_requires_alias_messages_to_be_arrays() {
    let catalog =
        BuiltinCatalog::new(&[("en-US", "required.aliases", Text("not an alias array"))]).unwrap();
    let key = I18nKey::new("required.aliases").unwrap();
    let required = I18nManifest {
        keys: BTreeSet::from([key.clone()]),
    };

    assert!(matches!(
        catalog.validate(&required, &BTreeSet::from([key])),
        Err(I18nBundleValidationError::AliasesNotArray { .. })
    ));
}

#[test]
fn builtin_nominal_validator_registration_propagates_duplicate_failure() {
    let mut builder = crate::node_system::registry::NodeRegistryBuilder::new();
    let project_columns = crate::node_system::protocol::TypeId::new(
        crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
    )
    .unwrap();
    builder
        .register_nominal_validator(
            project_columns.clone(),
            crate::node_system::protocol::TypeId::new("test.nominal.validator").unwrap(),
            1,
            |_| Ok(()),
        )
        .unwrap();

    assert!(matches!(
        register_builtin_nominal_validators_for_test(&mut builder),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::Registration(
                crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                    crate::node_system::registry::RegistryValidationError::DuplicateNominalValidator(
                        id,
                    ),
                ),
            )
        )) if id == project_columns
    ));
}

#[test]
fn builtin_catalog_invalid_i18n_key_preserves_semantic_source() {
    let expected_source = I18nKey::new("Bad Localization Key").unwrap_err();
    let error =
        BuiltinCatalog::new(&[("en-US", "Bad Localization Key", Text("invalid"))]).unwrap_err();

    assert!(matches!(
        &error,
        crate::node_system::protocol::ProtocolError::InvalidSemanticId { value, source }
            if value.as_ref() == "Bad Localization Key" && source == &expected_source
    ));
    assert!(
        error
            .source()
            .and_then(
                |source| source.downcast_ref::<crate::node_system::protocol::InvalidSemanticId>()
            )
            .is_some_and(|source| source == &expected_source)
    );
}

#[test]
fn builtin_assembly_rejects_invalid_semantic_id_with_source() {
    let expected_source = NodeTypeId::new("Bad Display ID").unwrap_err();
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidSemanticId("Bad Display ID"),
    )
    .err()
    .expect("invalid semantic ID must fail assembly");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidSemanticId { value, source }
        ) if value.as_ref() == "Bad Display ID" && source == &expected_source
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(
        assembly
            .source()
            .and_then(
                |source| source.downcast_ref::<crate::node_system::protocol::InvalidSemanticId>()
            )
            .is_some_and(|source| source == &expected_source)
    );
}

#[test]
fn builtin_assembly_rejects_invalid_protocol_without_fallback() {
    let expected_key = crate::node_system::protocol::PortKey::new("duplicate").unwrap();
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidProtocol("yssbi.test.invalid_protocol"),
    )
    .err()
    .expect("invalid protocol must fail assembly");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidProtocol {
                node_type,
                source: crate::node_system::protocol::ProtocolError::DuplicatePortKey(key),
            }
        ) if node_type.as_ref() == "yssbi.test.invalid_protocol" && key.as_str() == expected_key.as_str()
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(
        assembly
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::protocol::ProtocolError>())
            .is_some_and(|source| matches!(source, crate::node_system::protocol::ProtocolError::DuplicatePortKey(key) if key.as_str() == "duplicate"))
    );
}

#[test]
fn builtin_registry_revalidation_preserves_protocol_source_chain() {
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidRegistryProtocol,
    )
    .err()
    .expect("invalid frozen protocol must fail Registry validation");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(BuiltinAssemblyError::Registration(
            crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                crate::node_system::registry::RegistryValidationError::InvalidNodeProtocol {
                    node,
                    source: crate::node_system::protocol::ProtocolError::DuplicatePortKey(key),
                },
            ),
        )) if node.as_str() == "yssbi.constant.bool" && key.as_str() == "value"
    ));

    let assembly = error.source().unwrap();
    let registration = assembly.source().unwrap();
    let registry = registration.source().unwrap();
    let protocol = registry.source().unwrap();
    assert!(matches!(
        protocol.downcast_ref::<crate::node_system::protocol::ProtocolError>(),
        Some(crate::node_system::protocol::ProtocolError::DuplicatePortKey(key))
            if key.as_str() == "value"
    ));
}

#[test]
fn builtin_assembly_rejects_conflicting_localization() {
    let error =
        build_builtin_node_system_with_test_fault(BuiltinAssemblyTestFault::LocalizationConflict)
            .err()
            .expect("localization conflict must fail assembly");
    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::LocalizationConflict { locale, key }
        ) if locale.as_ref() == "en-US" && key.as_ref() == "nodes.test.title"
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(assembly.source().is_none());
}

#[test]
fn builtin_assembly_rejects_duplicate_registration() {
    let error =
        build_builtin_node_system_with_test_fault(BuiltinAssemblyTestFault::DuplicateRegistration)
            .err()
            .expect("duplicate registration must fail assembly");
    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(BuiltinAssemblyError::Registration(
            crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                crate::node_system::registry::RegistryValidationError::DuplicateNode(id),
            ),
        )) if id.as_str() == "yssbi.constant.bool"
    ));

    let assembly = std::error::Error::source(&error).unwrap();
    let registration = assembly.source().unwrap();
    assert!(
        registration
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::registry::RegistryValidationError>())
            .is_some_and(|source| matches!(source, crate::node_system::registry::RegistryValidationError::DuplicateNode(id) if id.as_str() == "yssbi.constant.bool"))
    );
}

#[test]
fn builtin_assembly_preserves_parameter_decimal_and_default_sources() {
    let builtin = build_builtin_node_system().unwrap();
    let constant = builtin
        .registry
        .protocol(&NodeTypeId::new("yssbi.constant.bool").unwrap())
        .unwrap();
    let parameter = constant.parameters.parameters[0].clone();
    let parameter_error = super::builtin::assembled_parameters(
        "yssbi.test.parameters",
        vec![parameter.clone(), parameter],
    )
    .unwrap_err();
    assert!(matches!(
        &parameter_error,
        BuiltinAssemblyError::InvalidParameterSchema { node_type, .. }
            if node_type.as_ref() == "yssbi.test.parameters"
    ));
    let parameter_source = parameter_error
        .source()
        .and_then(|source| {
            source.downcast_ref::<crate::node_system::protocol::ParameterSchemaError>()
        })
        .expect("parameter schema source");
    assert!(matches!(
        parameter_source,
        crate::node_system::protocol::ParameterSchemaError::DuplicateKey(error)
            if error.0.as_str() == "value"
    ));
    assert!(
        parameter_source
            .source()
            .and_then(|source| source
                .downcast_ref::<crate::node_system::protocol::DuplicateParameterKey>())
            .is_some_and(|source| source.0.as_str() == "value")
    );

    let decimal_error = super::builtin::assembled_decimal("yssbi.test.decimal", "01").unwrap_err();
    assert!(matches!(
        &decimal_error,
        BuiltinAssemblyError::InvalidDecimal { node_type, .. }
            if node_type.as_ref() == "yssbi.test.decimal"
    ));
    assert!(decimal_error
        .source()
        .and_then(|source| source.downcast_ref::<crate::node_system::protocol::InvalidDecimal>())
        .is_some_and(|source| source.to_string() == "'01' is not a canonical decimal"));

    let mut port = constant.interface.ports[0].clone();
    port.direction = PortDirection::Input;
    port.input_binding = Some(crate::node_system::protocol::InputBindingSpec {
        literal_policy: crate::node_system::protocol::LiteralPolicy::Allowed,
        default_value: Some(crate::node_system::protocol::TypedValue {
            value_type: TypeExpr::Concrete(
                crate::node_system::protocol::TypeId::new("core.string").unwrap(),
            ),
            value: crate::node_system::protocol::Value::String("wrong".into()),
        }),
    });
    port.consumption = Some(crate::node_system::protocol::InputConsumption::FullyMaterialized);
    port.production = None;
    let default_error = super::builtin::assembled_interface(
        "yssbi.test.default",
        vec![port],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(
        &default_error,
        BuiltinAssemblyError::InvalidDefaultBinding { node_type, .. }
            if node_type.as_ref() == "yssbi.test.default"
    ));
    assert!(
        default_error
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::protocol::ProtocolError>())
            .is_some_and(|source| matches!(
                source,
                crate::node_system::protocol::ProtocolError::InvalidPortContract { key, reason }
                    if key.as_str() == "value"
                        && *reason == "typed default does not match the port value type"
            ))
    );
}

#[test]
fn every_production_compiler_definition_is_required_by_builtin_i18n() {
    let (provider, _, _) = builtin_bundle_parts_for_test().unwrap();
    let missing = COMPILER_DIAGNOSTIC_DEFINITIONS
        .iter()
        .filter(|definition| {
            let message_key = I18nKey::new(definition.message_key).unwrap();
            !provider.i18n.keys.contains(&message_key)
        })
        .map(|definition| definition.message_key)
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "compiler diagnostic definitions missing from built-in i18n requirements:\n{}",
        missing.join("\n")
    );
}

#[test]
fn builtin_assembly_preserves_diagnostic_definition_error_source() {
    let source = CompilerDiagnosticDefinitionError::DuplicateCode {
        code: "compiler.test.duplicate".into(),
    };
    let error = BuiltinAssemblyError::DiagnosticDefinitions {
        source: source.clone(),
    };

    assert_eq!(
        error.to_string(),
        "built-in compiler diagnostic definitions are invalid: duplicate diagnostic code: compiler.test.duplicate"
    );
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<CompilerDiagnosticDefinitionError>()),
        Some(&source)
    );
}

#[test]
fn builtin_startup_rejects_missing_compiler_default_template() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let missing = I18nKey::new("diagnostics.compiler.node.scope_mismatch").unwrap();
    catalog.remove_message_for_test("en-US", &missing);

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::MissingDefaultLocale { keys }
        )) if keys.iter().any(|key| key.as_ref() == missing.as_str())
    ));
}

#[test]
fn builtin_startup_rejects_missing_default_locale_key() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let missing = provider.i18n.keys.iter().next().unwrap().clone();
    catalog.remove_message_for_test("en-US", &missing);

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::MissingDefaultLocale { keys }
        )) if keys.iter().any(|key| key.as_ref() == missing.as_str())
    ));
}

#[test]
fn builtin_startup_rejects_alias_stored_as_text() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let alias = alias_keys.iter().next().unwrap().clone();
    catalog.replace_message_for_test("en-US", alias.clone(), Text("malformed aliases"));

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::AliasesNotArray { locale, key }
        )) if locale.as_ref() == "en-US" && key.as_ref() == alias.as_str()
    ));
}

#[test]
fn builtin_startup_rejects_invalid_registry_before_returning_a_bundle() {
    let (mut provider, catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    provider.types[0].title_key = "missing.type.title".parse().unwrap();

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::Registration(_)
        ))
    ));
}

#[test]
fn builtin_startup_returns_one_validated_registry_and_catalog_bundle() {
    let bundle = build_builtin_node_system().unwrap();

    assert!(!bundle.registry.is_empty());
    assert_eq!(
        bundle
            .registry
            .node_provider(&NodeTypeId::new("yssbi.constant.bool").unwrap())
            .map(crate::node_system::protocol::ProviderId::as_str),
        Some("yssbi.builtin")
    );
    assert!(
        !bundle
            .catalog
            .localize(&bundle.registry, "en-US")
            .items
            .is_empty()
    );
}

#[test]
fn compiler_diagnostics_render_in_english_and_chinese() {
    let catalog = build_builtin_node_system().unwrap().catalog;
    let key = I18nKey::new("diagnostics.compiler.type.incompatible").unwrap();
    let arguments = DiagnosticArguments::from([
        (
            Box::<str>::from("actual_type"),
            Box::<str>::from("core.string"),
        ),
        (
            Box::<str>::from("expected_type"),
            Box::<str>::from("core.integer"),
        ),
    ]);

    assert_eq!(
        catalog
            .localization("en-US")
            .text(&key, &arguments)
            .as_ref(),
        "Type core.string is incompatible with core.integer."
    );
    assert_eq!(
        catalog
            .localization("zh-CN")
            .text(&key, &arguments)
            .as_ref(),
        "类型 core.string 与 core.integer 不兼容。"
    );
}

#[test]
fn diagnostic_rendering_does_not_rewrite_inserted_argument_placeholders() {
    let catalog = build_builtin_node_system().unwrap().catalog;
    let key = I18nKey::new("diagnostics.compiler.type.incompatible").unwrap();
    let arguments = DiagnosticArguments::from([
        (
            Box::<str>::from("actual_type"),
            Box::<str>::from("literal {expected_type}"),
        ),
        (
            Box::<str>::from("expected_type"),
            Box::<str>::from("core.integer"),
        ),
    ]);

    assert_eq!(
        catalog
            .localization("en-US")
            .text(&key, &arguments)
            .as_ref(),
        "Type literal {expected_type} is incompatible with core.integer."
    );
}
