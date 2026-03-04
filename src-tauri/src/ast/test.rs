//! 假设表达式解析器测试

#[cfg(test)]
mod tests {
    use crate::ast::types::{Expr, HypothesisExpr};
    use crate::ast::{parse_hypothesis, ParamRegistry};

    #[test]
    fn test_s_eq_0_1() {
        let result = parse_hypothesis("s = 0.1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Eq(e, k) => {
                assert!((*k - 0.1).abs() < 1e-10);
                assert!(matches!(e, Expr::Param(_)));
            }
            _ => panic!("expected Eq"),
        }
    }

    #[test]
    fn test_s_gt_0() {
        let result = parse_hypothesis("s > 0").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Gt(e, k) => {
                assert!((*k - 0.0).abs() < 1e-10);
                assert!(matches!(e, Expr::Param(_)));
            }
            _ => panic!("expected Gt"),
        }
    }

    #[test]
    fn test_s_ge_0_1() {
        let result = parse_hypothesis("s >= 0.1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Ge(e, k) => {
                assert!((*k - 0.1).abs() < 1e-10);
                assert!(matches!(e, Expr::Param(_)));
            }
            _ => panic!("expected Ge"),
        }
    }

    #[test]
    fn test_s_lt_0() {
        let result = parse_hypothesis("s < 0").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Lt(e, _) => assert!(matches!(e, Expr::Param(_))),
            _ => panic!("expected Lt"),
        }
    }

    #[test]
    fn test_s_minus_t_div_2_eq_1() {
        let mut reg = ParamRegistry::new();
        let result = crate::ast::parse_hypothesis_with_registry("(s - t)/2 = 1", &mut reg).unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Eq(e, k) => {
                assert!((*k - 1.0).abs() < 1e-10);
                assert!(matches!(e, Expr::Div(_, _)));
            }
            _ => panic!("expected Eq"),
        }
    }

    #[test]
    fn test_exp_s_eq_2() {
        let result = parse_hypothesis("exp(s) = 2").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            HypothesisExpr::Eq(e, k) => {
                assert!((*k - 2.0).abs() < 1e-10);
                assert!(matches!(e, Expr::Exp(_)));
            }
            _ => panic!("expected Eq"),
        }
    }

    #[test]
    fn test_chained_a_lt_b_lt_c() {
        let mut reg = ParamRegistry::new();
        let result =
            crate::ast::parse_hypothesis_with_registry("a < b < c", &mut reg).unwrap();
        assert_eq!(result.len(), 2);
        match &result[0] {
            HypothesisExpr::Lt(e, k) => {
                assert!((*k - 0.0).abs() < 1e-10);
                assert!(matches!(e, Expr::Sub(_, _)));
            }
            _ => panic!("expected Lt"),
        }
        match &result[1] {
            HypothesisExpr::Lt(e, k) => {
                assert!((*k - 0.0).abs() < 1e-10);
                assert!(matches!(e, Expr::Sub(_, _)));
            }
            _ => panic!("expected Lt"),
        }
    }

    #[test]
    fn test_param_registry_shared() {
        let mut reg = ParamRegistry::new();
        let _ = crate::ast::parse_hypothesis_with_registry("s = 0.1", &mut reg).unwrap();
        let _ = crate::ast::parse_hypothesis_with_registry("t > 0", &mut reg).unwrap();
        let s_id = reg.get_or_insert("s");
        let t_id = reg.get_or_insert("t");
        assert_ne!(s_id, t_id);
        assert_eq!(reg.get_or_insert("s"), s_id);
    }

    // ==================== 验证器测试 ====================

    use crate::ast::{validate_hypotheses, ValidationError};

    #[test]
    fn test_validate_eq_linear_ok() {
        let constraints = parse_hypothesis("s = 0.1").unwrap();
        assert!(validate_hypotheses(&constraints).is_ok());
    }

    #[test]
    fn test_validate_linear_div_ok() {
        let mut reg = ParamRegistry::new();
        let constraints =
            crate::ast::parse_hypothesis_with_registry("(s - t)/2 = 1", &mut reg).unwrap();
        assert!(validate_hypotheses(&constraints).is_ok());
    }

    #[test]
    fn test_validate_inequality_ok() {
        let constraints = parse_hypothesis("s > 0").unwrap();
        assert!(validate_hypotheses(&constraints).is_ok());
    }

    #[test]
    fn test_validate_mixed_direction_err() {
        let mut reg = ParamRegistry::new();
        let c1 = crate::ast::parse_hypothesis_with_registry("s > 0", &mut reg).unwrap();
        let c2 = crate::ast::parse_hypothesis_with_registry("t < 0", &mut reg).unwrap();
        let constraints = [c1[0].clone(), c2[0].clone()];
        let err = validate_hypotheses(&constraints).unwrap_err();
        assert!(matches!(err, ValidationError::MixedDirection(_)));
    }

    #[test]
    fn test_validate_div_by_var_err() {
        let mut reg = ParamRegistry::new();
        let constraints =
            crate::ast::parse_hypothesis_with_registry("s/t = 1", &mut reg).unwrap();
        let err = validate_hypotheses(&constraints).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("除法") || msg.contains("常数"));
    }

    #[test]
    fn test_validate_exp_err() {
        let constraints = parse_hypothesis("exp(s) = 2").unwrap();
        let err = validate_hypotheses(&constraints).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::ConstraintFailed { .. }
        ));
        let msg = err.to_string();
        assert!(msg.contains("exp"));
    }

    // ==================== 线性展开测试 ====================

    use crate::ast::{linear_expand, Alternative, HypothesisSpec, LinearConstraintKind, TestMethod};

    #[test]
    fn test_linear_expand_s_eq_0_1() {
        let constraints = parse_hypothesis("s = 0.1").unwrap();
        let spec = linear_expand(&constraints).unwrap();
        assert_eq!(spec.test_method, TestMethod::TTest);
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind: _ } => {
                assert_eq!(r.shape(), &[1, 1]);
                assert!((r[[0, 0]] - 1.0).abs() < 1e-10);
                assert!((r_vec[0] - 0.1).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
    }

    #[test]
    fn test_linear_expand_s_minus_t_div_2_eq_1() {
        let mut reg = ParamRegistry::new();
        let constraints =
            crate::ast::parse_hypothesis_with_registry("(s - t)/2 = 1", &mut reg).unwrap();
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind: _ } => {
                assert_eq!(r.shape(), &[1, 2]);
                assert!((r[[0, 0]] - 0.5).abs() < 1e-10);
                assert!((r[[0, 1]] - (-0.5)).abs() < 1e-10);
                assert!((r_vec[0] - 1.0).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
    }

    #[test]
    fn test_parse_comma_separated_constraints() {
        let constraints = parse_hypothesis("petal_width = -0.5626, petal_length = 0.7").unwrap();
        assert_eq!(constraints.len(), 2);
        // 验证 linear_expand 能正确展开（R 和 r_vec 对应 petal_width=-0.5626, petal_length=0.7）
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind: _ } => {
                assert_eq!(r.shape(), &[2, 2]);
                // 约束1: petal_width = -0.5626；约束2: petal_length = 0.7
                assert!((r_vec[0] - (-0.5626)).abs() < 1e-6 || (r_vec[1] - (-0.5626)).abs() < 1e-6);
                assert!((r_vec[0] - 0.7).abs() < 1e-6 || (r_vec[1] - 0.7).abs() < 1e-6);
            }
            _ => panic!("expected Linear"),
        }
    }

    #[test]
    fn test_linear_expand_two_constraints() {
        let mut reg = ParamRegistry::new();
        let c1 = crate::ast::parse_hypothesis_with_registry("s = 0.1", &mut reg).unwrap();
        let c2 = crate::ast::parse_hypothesis_with_registry("t = 0.2", &mut reg).unwrap();
        let constraints = [c1[0].clone(), c2[0].clone()];
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind: _ } => {
                assert_eq!(r.shape(), &[2, 2]);
                assert!((r[[0, 0]] - 1.0).abs() < 1e-10);
                assert!((r[[1, 1]] - 1.0).abs() < 1e-10);
                assert!((r_vec[0] - 0.1).abs() < 1e-10);
                assert!((r_vec[1] - 0.2).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
        assert_eq!(spec.alternative, Alternative::TwoSided);
        assert_eq!(spec.test_method, TestMethod::Wald);
    }

    #[test]
    fn test_linear_expand_s_gt_0() {
        let constraints = parse_hypothesis("s > 0").unwrap();
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind } => {
                assert_eq!(*kind, LinearConstraintKind::Ge);
                assert_eq!(r.shape(), &[1, 1]);
                assert!((r[[0, 0]] - 1.0).abs() < 1e-10);
                assert!((r_vec[0] - 0.0).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
    }

    #[test]
    fn test_linear_expand_s_lt_0() {
        let constraints = parse_hypothesis("s < 0").unwrap();
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind } => {
                assert_eq!(*kind, LinearConstraintKind::Ge);
                assert_eq!(r.shape(), &[1, 1]);
                assert!((r[[0, 0]] - (-1.0)).abs() < 1e-10);
                assert!((r_vec[0] - 0.0).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
    }

    #[test]
    fn test_linear_expand_s_gt_t() {
        let mut reg = ParamRegistry::new();
        let constraints =
            crate::ast::parse_hypothesis_with_registry("s > t", &mut reg).unwrap();
        let spec = linear_expand(&constraints).unwrap();
        match &spec.hypothesis {
            HypothesisSpec::Linear { r, r_vec, kind } => {
                assert_eq!(*kind, LinearConstraintKind::Ge);
                assert_eq!(r.shape(), &[1, 2]);
                // s > t => s - t > 0 => R = [1, -1], r = 0
                assert!((r[[0, 0]] - 1.0).abs() < 1e-10);
                assert!((r[[0, 1]] - (-1.0)).abs() < 1e-10);
                assert!((r_vec[0] - 0.0).abs() < 1e-10);
            }
            _ => panic!("expected Linear"),
        }
        assert_eq!(spec.alternative, Alternative::Greater);
    }

    #[test]
    fn test_validate_mixed_direction_eq_and_gt_err() {
        let mut reg = ParamRegistry::new();
        let c1 = crate::ast::parse_hypothesis_with_registry("s = 0.1", &mut reg).unwrap();
        let c2 = crate::ast::parse_hypothesis_with_registry("t > 0", &mut reg).unwrap();
        let constraints = [c1[0].clone(), c2[0].clone()];
        let err = validate_hypotheses(&constraints).unwrap_err();
        assert!(matches!(err, ValidationError::MixedDirection(_)));
    }
}
