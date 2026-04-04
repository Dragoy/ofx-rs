enum eOfxStatus {
	Unused = -1,
	OK = kOfxStatOK,
	ReplyDefault = kOfxStatReplyDefault,
	Failed = kOfxStatFailed,
	ErrFatal = kOfxStatErrFatal,
	ErrBadHandle = kOfxStatErrBadHandle,
	ErrBadIndex = kOfxStatErrBadIndex,
	ErrValue = kOfxStatErrValue,
	ErrUnknown = kOfxStatErrUnknown,
	ErrMemory = kOfxStatErrMemory,
	ErrUnsupported = kOfxStatErrUnsupported,
	ErrMissingHostFeature = kOfxStatErrMissingHostFeature,
};

#define kOfxImageEffectOpenGLRenderSuite "OfxImageEffectOpenGLRenderSuite"

#define kOfxImageEffectPropOpenGLEnabled "OfxImageEffectPropOpenGLEnabled"
#define kOfxImageEffectPropOpenGLTextureIndex "OfxImageEffectPropOpenGLTextureIndex"
#define kOfxImageEffectPropOpenGLTextureTarget "OfxImageEffectPropOpenGLTextureTarget"

#define kOfxImageEffectPropOpenCLRenderSupported "OfxImageEffectPropOpenCLRenderSupported"
#define kOfxImageEffectPropOpenCLEnabled "OfxImageEffectPropOpenCLEnabled"
#define kOfxImageEffectPropOpenCLCommandQueue "OfxImageEffectPropOpenCLCommandQueue"

#define kOfxImageEffectPropCudaRenderSupported "OfxImageEffectPropCudaRenderSupported"
#define kOfxImageEffectPropCudaEnabled "OfxImageEffectPropCudaEnabled"

#define kOfxImageEffectPropMetalRenderSupported "OfxImageEffectPropMetalRenderSupported"
#define kOfxImageEffectPropMetalEnabled "OfxImageEffectPropMetalEnabled"
#define kOfxImageEffectPropMetalCommandQueue "OfxImageEffectPropMetalCommandQueue"

#define kOfxImageEffectPropResolvePage "OfxImageEffectPropResolvePage"
#define kOfxImageEffectPropSrcFilePath "OfxImageEffectPropSrcFilePath"
#define kOfxImageEffectPropSrcFrame "OfxImageEffectPropSrcFrame"
