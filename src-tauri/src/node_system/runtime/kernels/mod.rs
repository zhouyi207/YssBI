mod core_nodes;
mod dataframe;
mod distribution;
mod plot;
mod statistics;

pub use core_nodes::{ConvertParameters, ConvertTarget};
pub(crate) use dataframe::dataframe_to_protocol_value_with_checkpoint;
pub use dataframe::{DataframeKernelParameters, dataframe_to_protocol_value};
pub(crate) use plot::PLOT_SINK;
pub use plot::{PlotKind, PlotPublishError, PlotSink, PlotSinkResource};
pub use statistics::StatisticsKernelParameters;

use super::{
    Kernel, KernelContext, KernelError, KernelRegistrationError, KernelRegistry, RuntimeValue,
};
use crate::node_system::plan::KernelHandle;

struct KernelRegistration {
    handle: KernelHandle,
    kernel: Box<dyn Kernel>,
}

#[derive(Default)]
pub(crate) struct KernelFragment {
    registrations: Vec<KernelRegistration>,
}

impl KernelFragment {
    pub(crate) fn register(&mut self, handle: &'static str, kernel: impl Kernel + 'static) {
        self.registrations.push(KernelRegistration {
            handle: KernelHandle::new(handle).expect("built-in kernel handles are valid"),
            kernel: Box::new(kernel),
        });
    }

    #[cfg(test)]
    pub(crate) fn handles(&self) -> impl Iterator<Item = &KernelHandle> {
        self.registrations
            .iter()
            .map(|registration| &registration.handle)
    }

    pub(crate) fn install(
        self,
        registry: &mut KernelRegistry,
    ) -> Result<(), KernelRegistrationError> {
        for registration in self.registrations {
            registry.register(registration.handle, BoxedKernel(registration.kernel))?;
        }
        Ok(())
    }
}

struct BoxedKernel(Box<dyn Kernel>);

impl Kernel for BoxedKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.0.execute(context, inputs)
    }
}

pub(crate) fn build_kernel_fragments() -> [KernelFragment; 5] {
    [
        core_nodes::build_kernel_fragment(),
        dataframe::build_kernel_fragment(),
        statistics::build_kernel_fragment(),
        distribution::build_kernel_fragment(),
        plot::build_kernel_fragment(),
    ]
}
