use ofx_sys::*;
use result::*;
use std::borrow::Borrow;
use std::sync::Arc;

#[derive(Clone)]
pub struct Suites {
    image_effect: Arc<OfxImageEffectSuiteV1>,
    property: Arc<OfxPropertySuiteV1>,
    parameter: Arc<OfxParameterSuiteV1>,
    memory: Arc<OfxMemorySuiteV1>,
    pub(crate) multi_thread: Arc<OfxMultiThreadSuiteV1>,
    message: Arc<OfxMessageSuiteV1>,
    message_v2: Option<Arc<OfxMessageSuiteV2>>,
    progress: Arc<OfxProgressSuiteV1>,
    progress_v2: Option<Arc<OfxProgressSuiteV2>>,
    time_line: Arc<OfxTimeLineSuiteV1>,
    parametric_parameter: Option<Arc<OfxParametricParameterSuiteV1>>,
    image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
}

macro_rules! suite_call {
	($function:ident in $suite:expr; $($arg:expr),*) => {
		unsafe { ($suite).$function.ok_or(Error::SuiteNotInitialized)?($($arg),*) }
	};
}

macro_rules! suite_fn {
	($($tail:tt)*) => { to_result!{suite_call!($($tail)*)} }
}

#[allow(clippy::too_many_arguments)]
impl Suites {
    pub fn new(
        image_effect: OfxImageEffectSuiteV1,
        property: OfxPropertySuiteV1,
        parameter: OfxParameterSuiteV1,
        memory: OfxMemorySuiteV1,
        multi_thread: OfxMultiThreadSuiteV1,
        message: OfxMessageSuiteV1,
        message_v2: Option<OfxMessageSuiteV2>,
        progress: OfxProgressSuiteV1,
        progress_v2: Option<OfxProgressSuiteV2>,
        time_line: OfxTimeLineSuiteV1,
        parametric_parameter: Option<OfxParametricParameterSuiteV1>,
        image_effect_opengl_render: Option<OfxImageEffectOpenGLRenderSuiteV1>,
    ) -> Self {
        Suites {
            image_effect: Arc::new(image_effect),
            property: Arc::new(property),
            parameter: Arc::new(parameter),
            memory: Arc::new(memory),
            multi_thread: Arc::new(multi_thread),
            message: Arc::new(message),
            message_v2: message_v2.map(Arc::new),
            progress: Arc::new(progress),
            progress_v2: progress_v2.map(Arc::new),
            time_line: Arc::new(time_line),
            parametric_parameter: parametric_parameter.map(Arc::new),
            image_effect_opengl_render: image_effect_opengl_render.map(Arc::new),
        }
    }

    pub fn image_effect(&self) -> Arc<OfxImageEffectSuiteV1> {
        self.image_effect.clone()
    }

    pub fn image_effect_opengl_render(&self) -> Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>> {
        self.image_effect_opengl_render.clone()
    }

    pub fn property(&self) -> Arc<OfxPropertySuiteV1> {
        self.property.clone()
    }

    pub fn parameter(&self) -> Arc<OfxParameterSuiteV1> {
        self.parameter.clone()
    }
}
