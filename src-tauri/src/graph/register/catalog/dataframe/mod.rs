impl NodeLayoutResolver for BreakDataFrameNode {
    fn resolve(
        &self,
        ctx: &dyn NodeLayoutContext,
        node: &NodeInstance,
    ) -> Vec<DynamicPinSpec> {

        let Some(PinSchema::DataFrame(df)) =
            ctx.input_schema(node.id, &PinRole::Data(DataRole::Input))
        else {
            return vec![];
        };

        df.columns.iter().enumerate().map(|(i, col)| {
            DynamicPinSpec {
                role: PinRole::Data(DataRole::Outputs(i)),
                kind: PinKind::Data,
                type_desc: PinTypeDesc::Concrete(DataType::Series(col.ty)),
                name: col.name.clone(),
            }
        }).collect()
    }
}
