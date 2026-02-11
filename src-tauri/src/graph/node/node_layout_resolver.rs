use crate::graph::PinDynamicSpec;
use crate::graph::NodeInstance;
use super::NodeLayoutContext;



pub trait NodeLayoutResolver {
    fn resolve(
        &self,
        ctx: &dyn NodeLayoutContext,
        node: &NodeInstance,
    ) -> Vec<PinDynamicSpec>;
}

// resolve(..) {
//     let df_schema = ctx.input_type_schema(node, DataRole::Input)?;

//     df_schema.columns.iter().enumerate().map(|(i, col)| {
//         DynamicPinSpec {
//             role: PinRole::Data(DataRole::Outputs(i)),
//             kind: PinKind::Data,
//             type_desc: PinDataType::concrete(DataType::Series(col.ty)),
//             name: col.name.clone(),
//         }
//     }).collect()
// }



// DataEvaluator {
//     ctx.set_output(DataRole::Outputs(i), series_value)
// }