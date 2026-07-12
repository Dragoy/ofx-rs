use enums::*;
use image::*;
use ofx_sys::*;
use property::*;
use result::*;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fmt;
use std::marker::PhantomData;
use std::os::raw::c_char;
use std::rc::Rc;
use std::sync::Arc;
use types::*;

const OFX_MESSAGE_FORMAT: &[u8] = b"%s\0";

#[derive(Debug, Clone)]
pub struct PropertySetHandle {
    inner: OfxPropertySetHandle,
    property: Arc<OfxPropertySuiteV1>,
}

impl PropertySetHandle {
    pub(crate) fn new(inner: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> Self {
        PropertySetHandle { inner, property }
    }

    pub(crate) fn empty() -> Self {
        panic!("Do not use, only for type validation testing");
        #[allow(deref_nullptr)]
        PropertySetHandle {
            inner: std::ptr::null::<OfxPropertySetStruct>() as *mut _,
            property: unsafe { Arc::new(*std::ptr::null()) },
        }
    }
}

#[derive(Clone)]
pub struct GenericPluginHandle {
    inner: VoidPtr,
    property: &'static OfxPropertySuiteV1,
}

#[derive(Clone)]
pub struct ImageEffectHost {
    inner: OfxPropertySetHandle,
    property: Arc<OfxPropertySuiteV1>,
}

impl ImageEffectHost {
    pub fn new(host: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> Self {
        ImageEffectHost {
            inner: host,
            property,
        }
    }
}

#[derive(Clone)]
pub struct ImageEffectHandle {
    inner: OfxImageEffectHandle,
    property: Arc<OfxPropertySuiteV1>,
    image_effect: Arc<OfxImageEffectSuiteV1>,
    image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
    parameter: Arc<OfxParameterSuiteV1>,
    message: Option<Arc<OfxMessageSuiteV1>>,
    message_v2: Option<Arc<OfxMessageSuiteV2>>,
}

#[derive(Clone)]
pub struct ClipInstance {
    inner: OfxImageClipHandle,
    inner_properties: OfxPropertySetHandle,
    property: Arc<OfxPropertySuiteV1>,
    image_effect: Arc<OfxImageEffectSuiteV1>,
    image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
}

#[derive(Clone)]
pub struct Image {
    inner: OfxPropertySetHandle,
    property: Arc<OfxPropertySuiteV1>,
    image_effect: Arc<OfxImageEffectSuiteV1>,
    image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
    is_texture: bool,
}

pub trait ParamHandleValue: Default + Clone {}
impl ParamHandleValue for Int {}
impl ParamHandleValue for Bool {}
impl ParamHandleValue for Double {}
impl ParamHandleValue for ParamRGBColourD {}
impl ParamHandleValue for ParamRGBAColourD {}
impl ParamHandleValue for String {}

pub trait ParamHandleValueDefault: ParamHandleValue + Default {}
impl ParamHandleValueDefault for Int {}
impl ParamHandleValueDefault for Double {}

#[derive(Clone)]
pub struct ParamHandle<T>
where
    T: ParamHandleValue,
{
    inner: OfxParamHandle,
    inner_properties: OfxPropertySetHandle,
    property: Arc<OfxPropertySuiteV1>,
    parameter: Arc<OfxParameterSuiteV1>,
    _type: PhantomData<T>,
}

#[derive(Clone)]
pub struct ParamSetHandle {
    inner: OfxParamSetHandle,
    property: Arc<OfxPropertySuiteV1>,
    parameter: Arc<OfxParameterSuiteV1>,
}

pub struct ParamEditGuard<'a> {
    param_set: &'a ParamSetHandle,
    active: bool,
}

// TODO: custom_derive?
macro_rules! trivial_debug {
	($($struct:ty),*) => {
		$(impl fmt::Debug for $struct {
			fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
				write!(f, "{} {{...}}", stringify!($struct))
			}
		})
		*
	}
}

trivial_debug!(
    ClipInstance,
    ImageEffectHandle,
    GenericPluginHandle,
    ImageEffectHost
);

impl ImageEffectHandle {
    pub fn new(
        inner: OfxImageEffectHandle,
        property: Arc<OfxPropertySuiteV1>,
        image_effect: Arc<OfxImageEffectSuiteV1>,
        image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
        parameter: Arc<OfxParameterSuiteV1>,
    ) -> Self {
        ImageEffectHandle {
            inner,
            property,
            image_effect,
            image_effect_opengl_render,
            parameter,
            message: None,
            message_v2: None,
        }
    }

    pub(crate) fn with_message_suites(
        mut self,
        message: Arc<OfxMessageSuiteV1>,
        message_v2: Option<Arc<OfxMessageSuiteV2>>,
    ) -> Self {
        self.message = Some(message);
        self.message_v2 = message_v2;
        self
    }
}

impl<T> ParamHandle<T>
where
    T: ParamHandleValue + Default,
{
    pub fn new(
        inner: OfxParamHandle,
        inner_properties: OfxPropertySetHandle,
        property: Arc<OfxPropertySuiteV1>,
        parameter: Arc<OfxParameterSuiteV1>,
    ) -> Self {
        ParamHandle {
            inner,
            inner_properties,
            property,
            parameter,
            _type: PhantomData,
        }
    }
}

impl<T> ParamHandle<T>
where
    T: ParamHandleValueDefault,
{
    pub fn get_value(&self) -> Result<T> {
        let mut value: T = T::default();
        suite_fn!(paramGetValue in self.parameter; self.inner, &mut value as *mut T)?;
        Ok(value)
    }

    pub fn get_value_at_time(&self, time: Time) -> Result<T> {
        let mut value: T = T::default();
        suite_fn!(paramGetValueAtTime in self.parameter; self.inner, time, &mut value as *mut T)?;
        Ok(value)
    }

    pub fn set_value(&self, value: T) -> Result<()> {
        suite_fn!(paramSetValue in self.parameter; self.inner, value)?;
        Ok(())
    }

    pub fn set_value_at_time(&self, time: Time, value: T) -> Result<()> {
        suite_fn!(paramSetValueAtTime in self.parameter; self.inner, time, value)?;
        Ok(())
    }

    pub fn get_num_keys(&self) -> Result<u32> {
        let mut value: u32 = 0;
        suite_fn!(paramGetNumKeys in self.parameter; self.inner, &mut value as *mut u32)?;
        Ok(value)
    }
    pub fn get_key_time(&self, nth_key: u32) -> Result<Time> {
        let mut time: Time = Time::default();
        suite_fn!(paramGetKeyTime in self.parameter; self.inner, nth_key, &mut time as *mut Time)?;
        Ok(time)
    }
    pub fn get_key_index(&self, time: Time, direction: i32) -> Result<i32> {
        let mut value: i32 = 0;
        suite_fn!(paramGetKeyIndex in self.parameter; self.inner, time, direction, &mut value as *mut i32)?;
        Ok(value)
    }
    pub fn delete_key(&self, time: Time) -> Result<()> {
        suite_fn!(paramDeleteKey in self.parameter; self.inner, time)?;
        Ok(())
    }
    pub fn delete_all_keys(&self) -> Result<()> {
        suite_fn!(paramDeleteAllKeys in self.parameter; self.inner)?;
        Ok(())
    }
}

impl ParamHandle<Bool> {
    pub fn get_value(&self) -> Result<Bool> {
        let mut value: Int = 0;
        suite_fn!(paramGetValue in self.parameter; self.inner, &mut value as *mut Int)?;
        Ok(value != 0)
    }

    pub fn get_value_at_time(&self, time: Time) -> Result<Bool> {
        let mut value: Int = 0;
        suite_fn!(paramGetValueAtTime in self.parameter; self.inner, time, &mut value as *mut Int)?;
        Ok(value != 0)
    }

    pub fn set_value(&self, value: Bool) -> Result<()> {
        suite_fn!(paramSetValue in self.parameter; self.inner, value as Int)?;
        Ok(())
    }

    pub fn set_value_at_time(&self, time: Time, value: Bool) -> Result<()> {
        suite_fn!(paramSetValueAtTime in self.parameter; self.inner, time, value as Int)?;
        Ok(())
    }
}

impl ParamHandle<ParamRGBColourD> {
    pub fn get_value(&self) -> Result<ParamRGBColourD> {
        let mut value: ParamRGBColourD = ParamRGBColourD::default();
        suite_fn!(paramGetValue in self.parameter; self.inner, &mut value.r as *mut Double, &mut value.g as *mut Double, &mut value.b as *mut Double)?;
        Ok(value)
    }

    pub fn get_value_at_time(&self, time: Time) -> Result<ParamRGBColourD> {
        let mut value: ParamRGBColourD = ParamRGBColourD::default();
        suite_fn!(paramGetValueAtTime in self.parameter; self.inner, time, &mut value.r as *mut Double, &mut value.g as *mut Double, &mut value.b as *mut Double)?;
        Ok(value)
    }

    pub fn set_value(&self, value: ParamRGBColourD) -> Result<()> {
        suite_fn!(paramSetValue in self.parameter; self.inner, value.r, value.g, value.b)?;
        Ok(())
    }

    pub fn set_value_at_time(&self, time: Time, value: ParamRGBColourD) -> Result<()> {
        suite_fn!(paramSetValueAtTime in self.parameter; self.inner, time, value.r, value.g, value.b)?;
        Ok(())
    }
}

impl ParamHandle<ParamRGBAColourD> {
    pub fn get_value(&self) -> Result<ParamRGBAColourD> {
        let mut value: ParamRGBAColourD = unsafe { std::mem::zeroed() };
        suite_fn!(paramGetValue in self.parameter; self.inner, &mut value.r as *mut Double, &mut value.g as *mut Double, &mut value.b as *mut Double, &mut value.a as *mut Double)?;
        Ok(value)
    }

    pub fn get_value_at_time(&self, time: Time) -> Result<ParamRGBAColourD> {
        let mut value: ParamRGBAColourD = unsafe { std::mem::zeroed() };
        suite_fn!(paramGetValueAtTime in self.parameter; self.inner, time, &mut value.r as *mut Double, &mut value.g as *mut Double, &mut value.b as *mut Double, &mut value.a as *mut Double)?;
        Ok(value)
    }

    pub fn set_value(&self, value: ParamRGBAColourD) -> Result<()> {
        suite_fn!(paramSetValue in self.parameter; self.inner, value.r, value.g, value.b, value.a)?;
        Ok(())
    }

    pub fn set_value_at_time(&self, time: Time, value: ParamRGBAColourD) -> Result<()> {
        suite_fn!(paramSetValueAtTime in self.parameter; self.inner, time, value.r, value.g, value.b, value.a)?;
        Ok(())
    }
}

impl ParamHandle<String> {
    pub fn get_value(&self) -> Result<String> {
        let mut value: CharPtr = std::ptr::null();
        suite_fn!(paramGetValue in self.parameter; self.inner, &mut value as *mut CharPtr)?;
        if value.is_null() {
            Ok(String::new())
        } else {
            unsafe { std::ffi::CStr::from_ptr(value) }
                .to_str()
                .map(|s| s.to_owned())
                .map_err(|e| e.into())
        }
    }

    pub fn get_value_at_time(&self, time: Time) -> Result<String> {
        let mut value: CharPtr = std::ptr::null();
        suite_fn!(paramGetValueAtTime in self.parameter; self.inner, time, &mut value as *mut CharPtr)?;
        if value.is_null() {
            Ok(String::new())
        } else {
            unsafe { std::ffi::CStr::from_ptr(value) }
                .to_str()
                .map(|s| s.to_owned())
                .map_err(|e| e.into())
        }
    }

    pub fn set_value(&self, value: String) -> Result<()> {
        let ptr = std::ffi::CString::new(value)?;
        suite_fn!(paramSetValue in self.parameter; self.inner, ptr.as_ptr() as *const CharPtr)?;
        Ok(())
    }

    pub fn set_value_at_time(&self, time: Time, value: String) -> Result<()> {
        let ptr = std::ffi::CString::new(value)?;
        suite_fn!(paramSetValueAtTime in self.parameter; self.inner, time, ptr.as_ptr() as *const CharPtr)?;
        Ok(())
    }
}

impl ClipInstance {
    pub fn new(
        inner: OfxImageClipHandle,
        inner_properties: OfxPropertySetHandle,
        property: Arc<OfxPropertySuiteV1>,
        image_effect: Arc<OfxImageEffectSuiteV1>,
        image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
    ) -> Self {
        ClipInstance {
            inner,
            inner_properties,
            property,
            image_effect,
            image_effect_opengl_render,
        }
    }

    pub fn get_region_of_definition(&self, time: Time) -> Result<RectD> {
        let mut value = RectD {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
        };
        suite_fn!(clipGetRegionOfDefinition in self.image_effect; self.inner, time, &mut value as *mut RectD)?;
        Ok(value)
    }

    pub fn get_image_mut(&mut self, time: Time) -> Result<Rc<RefCell<Image>>> {
        self.get_image_rect_mut(time, None)
    }

    pub fn get_image(&self, time: Time) -> Result<Rc<Image>> {
        self.get_image_rect(time, None)
    }

    pub fn get_image_rect(&self, time: Time, region: Option<RectD>) -> Result<Rc<Image>> {
        let mut image: OfxPropertySetHandle = std::ptr::null_mut();
        let region_ptr = region
            .as_ref()
            .map(|m| m as *const RectD)
            .unwrap_or(std::ptr::null());
        suite_fn!(clipGetImage in self.image_effect; self.inner, time, region_ptr, &mut image as *mut OfxPropertySetHandle)?;
        Ok(Rc::new(Image::new(
            image,
            self.property.clone(),
            self.image_effect.clone(),
            self.image_effect_opengl_render.clone(),
            false,
        )))
    }

    pub fn load_texture(&self, time: Time, region: Option<RectD>) -> Result<Rc<Image>> {
        if let Some(suite) = self.image_effect_opengl_render.as_ref() {
            let mut image: OfxPropertySetHandle = std::ptr::null_mut();
            let region_ptr = region
                .as_ref()
                .map(|m| m as *const RectD)
                .unwrap_or(std::ptr::null());
            suite_fn!(clipLoadTexture in suite; self.inner, time, std::ptr::null(), region_ptr, &mut image as *mut OfxPropertySetHandle)?;
            Ok(Rc::new(Image::new(
                image,
                self.property.clone(),
                self.image_effect.clone(),
                self.image_effect_opengl_render.clone(),
                true,
            )))
        } else {
            Err(Error::InvalidSuite)
        }
    }
    pub fn load_texture_mut(
        &self,
        time: Time,
        region: Option<RectD>,
    ) -> Result<Rc<RefCell<Image>>> {
        if let Some(suite) = self.image_effect_opengl_render.as_ref() {
            let mut image: OfxPropertySetHandle = std::ptr::null_mut();
            let region_ptr = region
                .as_ref()
                .map(|m| m as *const RectD)
                .unwrap_or(std::ptr::null());
            suite_fn!(clipLoadTexture in suite; self.inner, time, std::ptr::null(), region_ptr, &mut image as *mut OfxPropertySetHandle)?;
            Ok(Rc::new(RefCell::new(Image::new(
                image,
                self.property.clone(),
                self.image_effect.clone(),
                self.image_effect_opengl_render.clone(),
                true,
            ))))
        } else {
            Err(Error::InvalidSuite)
        }
    }

    pub fn get_image_rect_mut(
        &mut self,
        time: Time,
        region: Option<RectD>,
    ) -> Result<Rc<RefCell<Image>>> {
        let mut image: OfxPropertySetHandle = std::ptr::null_mut();
        let region_ptr = region
            .as_ref()
            .map(|m| m as *const RectD)
            .unwrap_or(std::ptr::null());
        suite_fn!(clipGetImage in self.image_effect; self.inner, time, region_ptr, &mut image as *mut OfxPropertySetHandle)?;
        Ok(Rc::new(RefCell::new(Image::new(
            image,
            self.property.clone(),
            self.image_effect.clone(),
            self.image_effect_opengl_render.clone(),
            false,
        ))))
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        self.drop_image()
            .expect("Unable to drop image handle. This is likely a bug");
    }
}

impl Image {
    pub fn new(
        inner: OfxPropertySetHandle,
        property: Arc<OfxPropertySuiteV1>,
        image_effect: Arc<OfxImageEffectSuiteV1>,
        image_effect_opengl_render: Option<Arc<OfxImageEffectOpenGLRenderSuiteV1>>,
        is_texture: bool,
    ) -> Self {
        Image {
            inner,
            property,
            image_effect,
            image_effect_opengl_render,
            is_texture,
        }
    }

    pub fn get_descriptor<T>(&self) -> Result<ImageDescriptor<'_, T>>
    where
        T: PixelFormat,
    {
        let bounds = self.get_bounds()?;
        let row_bytes = self.get_row_bytes()?;
        let ptr = self.get_data()?;

        Ok(ImageDescriptor::new(bounds, row_bytes, ptr))
    }

    fn get_descriptor_mut<T>(&mut self) -> Result<ImageDescriptorMut<'_, T>>
    where
        T: PixelFormat,
    {
        let bounds = self.get_bounds()?;
        let row_bytes = self.get_row_bytes()?;
        let mut ptr = self.get_data()?;

        Ok(ImageDescriptorMut::new(bounds, row_bytes, ptr))
    }

    pub fn get_tiles_mut<T>(&mut self, count: usize) -> Result<Vec<ImageTileMut<'_, T>>>
    where
        T: PixelFormat,
    {
        let bounds = self.get_bounds()?;
        let row_bytes = self.get_row_bytes()?;
        let mut ptr = self.get_data()?;

        Ok(ImageDescriptorMut::new(bounds, row_bytes, ptr).into_tiles(count))
    }

    fn drop_image(&mut self) -> Result<()> {
        debug!("Releasing data for ImageHandle {:?}", self.inner);
        if self.is_texture {
            if let Some(suite) = self.image_effect_opengl_render.as_ref() {
                suite_fn!(clipFreeTexture in suite; self.inner)?;
                Ok(())
            } else {
                Err(Error::InvalidSuite)
            }
        } else {
            suite_fn!(clipReleaseImage in self.image_effect; self.inner)
        }
    }
}

impl HasProperties<ClipDescriptor> for ClipInstance {
    fn properties(&self) -> Result<ClipDescriptor> {
        Ok(ClipDescriptor::new(
            self.inner_properties,
            self.property.clone(),
        ))
    }
}

trait IsPropertiesNewType {
    fn wrap(inner: PropertySetHandle) -> Self;
}

pub trait PropertiesNewTypeConstructor {
    fn build(host: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> Self;
}

#[inline]
pub fn build_typed<T>(host: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> T
where
    T: PropertiesNewTypeConstructor,
{
    T::build(host, property)
}

macro_rules! properties_newtype {
    ($name:ident) => {
        #[derive(Clone)]
        pub struct $name(PropertySetHandle);

        impl IsPropertiesNewType for $name {
            fn wrap(inner: PropertySetHandle) -> Self {
                $name(inner)
            }
        }

        impl PropertiesNewTypeConstructor for $name {
            fn build(host: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> Self {
                $name::new(host, property)
            }
        }

        impl $name {
            pub fn new(host: OfxPropertySetHandle, property: Arc<OfxPropertySuiteV1>) -> Self {
                $name(PropertySetHandle::new(host, property))
            }
        }

        impl<'a> AsProperties for $name {
            fn handle(&self) -> OfxPropertySetHandle {
                self.0.inner
            }
            fn suite(&self) -> *const OfxPropertySuiteV1 {
                self.0.property.borrow() as *const _
            }
        }

        trivial_debug!($name);
    };
}

properties_newtype!(HostProperties);
properties_newtype!(EffectDescriptor);
properties_newtype!(EffectInstance);
properties_newtype!(ClipDescriptor);

properties_newtype!(DescribeInContextInArgs);

properties_newtype!(GetRegionOfDefinitionInArgs);
properties_newtype!(GetRegionOfDefinitionOutArgs);

properties_newtype!(GetRegionsOfInterestInArgs);
properties_newtype!(GetRegionsOfInterestOutArgs);

properties_newtype!(GetFramesNeededInArgs);
properties_newtype!(GetFramesNeededOutArgs);

properties_newtype!(GetClipPreferencesOutArgs);

properties_newtype!(IsIdentityInArgs);
properties_newtype!(IsIdentityOutArgs);

properties_newtype!(BeginInstanceChangedInArgs);

properties_newtype!(InstanceChangedInArgs);
properties_newtype!(InstanceChangedOutArgs);

properties_newtype!(EndInstanceChangedInArgs);
properties_newtype!(EndInstanceChangedOutArgs);

properties_newtype!(GetTimeDomainOutArgs);

properties_newtype!(BeginSequenceRenderInArgs);
properties_newtype!(RenderInArgs);
properties_newtype!(EndSequenceRenderInArgs);

properties_newtype!(ParamDouble);
properties_newtype!(ParamInt);
properties_newtype!(ParamBoolean);
properties_newtype!(ParamRGB);
properties_newtype!(ParamRGBA);
properties_newtype!(ParamString);
properties_newtype!(ParamPage);
properties_newtype!(ParamGroup);
properties_newtype!(ParamPushButton);
properties_newtype!(ParamChoice);

properties_newtype!(ParameterSet);

impl DescribeInContextInArgs {}

impl HasProperties<EffectInstance> for ImageEffectHandle {
    fn properties(&self) -> Result<EffectInstance> {
        let property_set_handle = {
            let mut property_set_handle = std::ptr::null_mut();

            suite_fn!(getPropertySet in self.image_effect; self.inner, &mut property_set_handle as *mut _)?;

            property_set_handle
        };
        Ok(EffectInstance(PropertySetHandle::new(
            property_set_handle,
            self.property.clone(),
        )))
    }
}

impl HasProperties<EffectDescriptor> for ImageEffectHandle {
    fn properties(&self) -> Result<EffectDescriptor> {
        let property_set_handle = {
            let mut property_set_handle = std::ptr::null_mut();

            suite_fn!(getPropertySet in self.image_effect; self.inner, &mut property_set_handle as *mut _)?;

            property_set_handle
        };
        Ok(EffectDescriptor(PropertySetHandle::new(
            property_set_handle,
            self.property.clone(),
        )))
    }
}

impl ImageEffectHandle {
    pub fn identity(&self) -> usize {
        self.inner as usize
    }

    pub fn message_error(&self, id: &CStr, message: &CStr) -> Result<()> {
        let message_suite = self.message.as_ref().ok_or(Error::InvalidSuite)?;
        suite_fn!(message in message_suite;
            self.inner as *mut _,
            kOfxMessageError.as_ptr() as *const i8,
            id.as_ptr(),
            OFX_MESSAGE_FORMAT.as_ptr() as *const i8,
            message.as_ptr())?;
        Ok(())
    }

    pub fn set_persistent_error(&self, id: &CStr, message: &CStr) -> Result<bool> {
        let Some(message_suite) = self.message_v2.as_ref() else {
            return Ok(false);
        };
        let Some(set_persistent_message) = message_suite.setPersistentMessage else {
            return Ok(false);
        };
        to_result!(unsafe {
            set_persistent_message(
                self.inner as *mut _,
                kOfxMessageError.as_ptr() as *const i8,
                id.as_ptr(),
                OFX_MESSAGE_FORMAT.as_ptr() as *const i8,
                message.as_ptr(),
            )
        })?;
        Ok(true)
    }

    pub fn clear_persistent_message(&self) -> Result<bool> {
        let Some(message_suite) = self.message_v2.as_ref() else {
            return Ok(false);
        };
        let Some(clear_persistent_message) = message_suite.clearPersistentMessage else {
            return Ok(false);
        };
        to_result!(unsafe { clear_persistent_message(self.inner as *mut _) })?;
        Ok(true)
    }

    fn clip_define(&self, clip_name: &[u8]) -> Result<ClipDescriptor> {
        let property_set_handle = {
            let mut property_set_handle = std::ptr::null_mut();
            suite_fn!(clipDefine in self.image_effect;
				self.inner, clip_name.as_ptr() as *const i8, &mut property_set_handle as *mut _)?;
            property_set_handle
        };
        Ok(ClipDescriptor(PropertySetHandle::new(
            property_set_handle,
            self.property.clone(),
        )))
    }

    fn clip_get_handle(&self, clip_name: &[u8]) -> Result<ClipInstance> {
        let (clip_handle, clip_properties) = {
            let mut clip_handle = std::ptr::null_mut();
            let mut clip_properties = std::ptr::null_mut();
            suite_fn!(clipGetHandle in self.image_effect;
				self.inner, clip_name.as_ptr() as *const i8, &mut clip_handle as *mut _, &mut clip_properties as *mut _)?;
            (clip_handle, clip_properties)
        };
        Ok(ClipInstance::new(
            clip_handle,
            clip_properties,
            self.property.clone(),
            self.image_effect.clone(),
            self.image_effect_opengl_render.clone(),
        ))
    }

    pub fn abort(&self) -> Result<Bool> {
        Ok(suite_call!(abort in self.image_effect; self.inner) != 0)
    }

    pub fn parameter_set(&self) -> Result<ParamSetHandle> {
        let parameters_set_handle = {
            let mut parameters_set_handle = std::ptr::null_mut();
            suite_fn!(getParamSet in self.image_effect; self.inner, &mut parameters_set_handle as *mut _)?;
            parameters_set_handle
        };
        Ok(ParamSetHandle::new(
            parameters_set_handle,
            self.parameter.clone(),
            self.property.clone(),
        ))
    }

    pub fn get_output_clip(&self) -> Result<ClipInstance> {
        self.clip_get_handle(ofx_sys::kOfxImageEffectOutputClipName)
    }

    pub fn get_simple_input_clip(&self) -> Result<ClipInstance> {
        self.clip_get_handle(ofx_sys::kOfxImageEffectSimpleSourceClipName)
    }

    pub fn get_clip(&self, name: &str) -> Result<ClipInstance> {
        let str_buf = CString::new(name)?.into_bytes_with_nul();
        self.clip_get_handle(&str_buf)
    }

    pub fn new_output_clip(&self) -> Result<ClipDescriptor> {
        self.clip_define(ofx_sys::kOfxImageEffectOutputClipName)
    }

    pub fn new_simple_input_clip(&self) -> Result<ClipDescriptor> {
        self.clip_define(ofx_sys::kOfxImageEffectSimpleSourceClipName)
    }

    pub fn new_clip(&self, name: &str) -> Result<ClipDescriptor> {
        let str_buf = CString::new(name)?.into_bytes_with_nul();
        self.clip_define(&str_buf)
    }

    unsafe fn get_pointer(&self) -> Result<*mut [u8]> {
        Err(Error::Unimplemented)
    }

    pub fn set_instance_data<T>(&mut self, data: T) -> Result<()>
    where
        T: Sized,
    {
        let mut effect_props: EffectInstance = self.properties()?;
        let data_box = Box::new(data);
        let data_ptr = Box::into_raw(data_box);
        let status = suite_fn!(propSetPointer in self.property;
			effect_props.0.inner, kOfxPropInstanceData.as_ptr() as *const i8, 0, data_ptr as *mut _);
        if status.is_err() {
            unsafe {
                Box::from_raw(data_ptr);
            }
        }
        status
    }

    fn get_instance_data_ptr(&self) -> Result<VoidPtrMut> {
        let mut effect_props: EffectInstance = self.properties()?;
        let mut data_ptr = std::ptr::null_mut();
        to_result! { suite_call!(propGetPointer in self.property;
           effect_props.0.inner, kOfxPropInstanceData.as_ptr() as *const i8, 0, &mut data_ptr)
        => data_ptr }
    }

    // TODO: this is not safe enough
    pub fn get_instance_data<T>(&self) -> Result<&mut T>
    where
        T: Sized,
    {
        unsafe {
            let mut ptr = self.get_instance_data_ptr()?;
            let mut reference = ptr as *mut T;
            Ok(&mut *reference)
        }
    }

    pub fn drop_instance_data(&mut self) -> Result<()> {
        unsafe {
            let mut ptr = self.get_instance_data_ptr()?;
            if !ptr.is_null() {
                Box::from_raw(ptr);
            }
        }
        Ok(())
    }
}

impl ParamSetHandle {
    pub fn new(
        inner: OfxParamSetHandle,
        parameter: Arc<OfxParameterSuiteV1>,
        property: Arc<OfxPropertySuiteV1>,
    ) -> Self {
        ParamSetHandle {
            inner,
            parameter,
            property,
        }
    }

    pub fn edit_guard(&self, name: &str) -> Result<ParamEditGuard<'_>> {
        self.edit_begin(name)?;
        Ok(ParamEditGuard {
            param_set: self,
            active: true,
        })
    }

    pub fn edit_begin(&self, name: &str) -> Result<()> {
        let name_buf = CString::new(name)?;
        suite_fn!(paramEditBegin in self.parameter; self.inner, name_buf.as_ptr())?;
        Ok(())
    }

    pub fn edit_end(&self) -> Result<()> {
        suite_fn!(paramEditEnd in self.parameter; self.inner)?;
        Ok(())
    }

    fn param_define<T>(&mut self, param_type: ParamType, name: &str) -> Result<T>
    where
        T: IsPropertiesNewType,
    {
        let name_buf = CString::new(name)?.into_bytes_with_nul();
        let property_set_handle = {
            let mut property_set_handle = std::ptr::null_mut();
            suite_fn!(paramDefine in self.parameter;
				self.inner, param_type.as_ptr() as *const _, name_buf.as_ptr() as *const _, &mut property_set_handle as *mut _)?;

            property_set_handle
        };
        Ok(T::wrap(PropertySetHandle::new(
            property_set_handle,
            self.property.clone(),
        )))
    }

    pub fn parameter<T>(&self, name: &str) -> Result<ParamHandle<T>>
    where
        T: ParamHandleValue,
    {
        let name_buf = CString::new(name)?.into_bytes_with_nul();
        let (param_handle, param_properties) = {
            let mut param_handle = std::ptr::null_mut();
            let mut param_properties = std::ptr::null_mut();
            suite_fn!(paramGetHandle in self.parameter;
				self.inner, name_buf.as_ptr() as *const _, &mut param_handle as *mut _, &mut param_properties as *mut _)?;
            (param_handle, param_properties)
        };
        Ok(ParamHandle::new(
            param_handle,
            param_properties,
            self.property.clone(),
            self.parameter.clone(),
        ))
    }

    pub fn parameter_group_properties(&self, name: &str) -> Result<ParamGroup> {
        let name_buf = CString::new(name)?.into_bytes_with_nul();
        let param_properties = {
            let mut param_handle = std::ptr::null_mut();
            let mut param_properties = std::ptr::null_mut();
            suite_fn!(paramGetHandle in self.parameter;
				self.inner, name_buf.as_ptr() as *const _, &mut param_handle as *mut _, &mut param_properties as *mut _)?;
            param_properties
        };
        Ok(ParamGroup::wrap(PropertySetHandle::new(
            param_properties,
            self.property.clone(),
        )))
    }

    pub fn param_define_double(&mut self, name: &str) -> Result<ParamDouble> {
        self.param_define(ParamType::Double, name)
    }

    pub fn param_define_int(&mut self, name: &str) -> Result<ParamInt> {
        self.param_define(ParamType::Integer, name)
    }

    pub fn param_define_boolean(&mut self, name: &str) -> Result<ParamBoolean> {
        self.param_define(ParamType::Boolean, name)
    }

    pub fn param_define_rgb(&mut self, name: &str) -> Result<ParamRGB> {
        self.param_define(ParamType::RGB, name)
    }

    pub fn param_define_rgba(&mut self, name: &str) -> Result<ParamRGBA> {
        self.param_define(ParamType::RGBA, name)
    }

    pub fn param_define_string(&mut self, name: &str) -> Result<ParamString> {
        self.param_define(ParamType::String, name)
    }

    pub fn param_define_group(&mut self, name: &str) -> Result<ParamGroup> {
        self.param_define(ParamType::Group, name)
    }

    pub fn param_define_page(&mut self, name: &str) -> Result<ParamPage> {
        self.param_define(ParamType::Page, name)
    }

    pub fn param_define_button(&mut self, name: &str) -> Result<ParamPushButton> {
        self.param_define(ParamType::PushButton, name)
    }

    pub fn param_define_choice(&mut self, name: &str) -> Result<ParamChoice> {
        self.param_define(ParamType::Choice, name)
    }
}

impl ParamEditGuard<'_> {
    pub fn finish(mut self) -> Result<()> {
        self.active = false;
        self.param_set.edit_end()
    }
}

impl Drop for ParamEditGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.param_set.edit_end();
        }
    }
}

impl AsProperties for ImageEffectHost {
    fn handle(&self) -> OfxPropertySetHandle {
        self.inner
    }
    fn suite(&self) -> *const OfxPropertySuiteV1 {
        self.property.borrow() as *const _
    }
}

impl AsProperties for ClipInstance {
    fn handle(&self) -> OfxPropertySetHandle {
        self.inner_properties
    }
    fn suite(&self) -> *const OfxPropertySuiteV1 {
        self.property.borrow() as *const _
    }
}

impl AsProperties for Image {
    fn handle(&self) -> OfxPropertySetHandle {
        self.inner
    }
    fn suite(&self) -> *const OfxPropertySuiteV1 {
        self.property.borrow() as *const _
    }
}

impl<T> AsProperties for ParamHandle<T>
where
    T: ParamHandleValue,
{
    fn handle(&self) -> OfxPropertySetHandle {
        self.inner_properties
    }
    fn suite(&self) -> *const OfxPropertySuiteV1 {
        self.property.borrow() as *const _
    }
}

mod tests {
    use super::*;
    use property;
    use property::*;

    // do not run, just compile!
    fn prop_host() {
        let mut handle = EffectInstance(PropertySetHandle::empty());

        handle.get::<property::TypeProp::Property>();
        handle.get::<property::IsBackground::Property>();
    }
}
