//! Stable inventory for statistical node families.

/// Operation performed within a method family, independent of catalog placement.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Fit,
    Summary,
    Predict,
    Test,
}

/// Statistical method identity shared by its fit, summary, and prediction nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Family {
    Adf,
    Linear,
    Iv2sls,
    IvLiml,
    Logit,
    Probit,
    Prais,
    Panel,
    PanelDid,
    Var,
    Vec,
    VecRank,
}

#[derive(Clone, Copy)]
pub(super) struct NodeSpec {
    pub id: &'static str,
    pub family: Family,
    pub stage: Stage,
}

pub(super) const NODES: &[NodeSpec] = &[
    node(
        "yssbi.statistics.panel.predict",
        Family::Panel,
        Stage::Predict,
    ),
    node("yssbi.statistics.panel.compare", Family::Panel, Stage::Fit),
    node(
        "yssbi.statistics.panel.did.randomization",
        Family::PanelDid,
        Stage::Test,
    ),
    node("yssbi.statistics.adf.test", Family::Adf, Stage::Test),
    node("yssbi.statistics.linear.fit", Family::Linear, Stage::Fit),
    node(
        crate::LINEAR_SUMMARY_NODE_TYPE,
        Family::Linear,
        Stage::Summary,
    ),
    node("yssbi.statistics.iv.2sls.fit", Family::Iv2sls, Stage::Fit),
    node(
        "yssbi.statistics.iv.2sls.summary",
        Family::Iv2sls,
        Stage::Summary,
    ),
    node("yssbi.statistics.iv.liml.fit", Family::IvLiml, Stage::Fit),
    node(
        "yssbi.statistics.iv.liml.summary",
        Family::IvLiml,
        Stage::Summary,
    ),
    node("yssbi.statistics.logit.fit", Family::Logit, Stage::Fit),
    node(
        "yssbi.statistics.logit.summary",
        Family::Logit,
        Stage::Summary,
    ),
    node("yssbi.statistics.panel.fit", Family::Panel, Stage::Fit),
    node(
        "yssbi.statistics.panel.summary",
        Family::Panel,
        Stage::Summary,
    ),
    node(
        "yssbi.statistics.panel.did.twfe",
        Family::PanelDid,
        Stage::Fit,
    ),
    node("yssbi.statistics.prais.fit", Family::Prais, Stage::Fit),
    node(
        "yssbi.statistics.prais.summary",
        Family::Prais,
        Stage::Summary,
    ),
    node(
        "yssbi.statistics.linear.predict",
        Family::Linear,
        Stage::Predict,
    ),
    node(
        "yssbi.statistics.logit.predict",
        Family::Logit,
        Stage::Predict,
    ),
    node(
        "yssbi.statistics.probit.predict",
        Family::Probit,
        Stage::Predict,
    ),
    node("yssbi.statistics.probit.fit", Family::Probit, Stage::Fit),
    node(
        "yssbi.statistics.probit.summary",
        Family::Probit,
        Stage::Summary,
    ),
    node("yssbi.statistics.var.lag_order", Family::Var, Stage::Test),
    node("yssbi.statistics.var.fit", Family::Var, Stage::Fit),
    node("yssbi.statistics.var.summary", Family::Var, Stage::Summary),
    node("yssbi.statistics.vec.fit", Family::Vec, Stage::Fit),
    node("yssbi.statistics.vec.summary", Family::Vec, Stage::Summary),
    node(
        "yssbi.statistics.vec.rank_test",
        Family::VecRank,
        Stage::Test,
    ),
];

const fn node(id: &'static str, family: Family, stage: Stage) -> NodeSpec {
    NodeSpec { id, family, stage }
}
