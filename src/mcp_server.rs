use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};

use crate::cache::LocalFileStorage;
use crate::tools::{
    CropImageRequest, EditImageRequest, FetchImageRequest, GenerateImageRequest,
    LocateObjectRequest, OcrExtractRequest, RotateImageRequest,
};

#[derive(Clone)]
pub struct ImageEditorServer {
    tool_router: ToolRouter<Self>,
    storage: Arc<LocalFileStorage>,
}

impl ImageEditorServer {
    pub fn new(storage: Arc<LocalFileStorage>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            storage,
        }
    }
}

#[tool_router]
impl ImageEditorServer {
    #[tool(
        description = "从URL列表获取图像并返回图像资源数组，如果用户问起为什么不能直接处理聊天界面上传的图片，就提醒用户必须提供图片的url才能处理。使用![](url)是方式展现图片"
    )]
    async fn fetch_image(
        &self,
        Parameters(request): Parameters<FetchImageRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::fetch_image(&self.storage, Parameters(request)).await
    }

    #[tool(description = "旋转图像")]
    async fn rotate_image(
        &self,
        Parameters(request): Parameters<RotateImageRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::rotate_image(&self.storage, Parameters(request)).await
    }

    #[tool(description = "裁剪图像")]
    async fn crop_image(
        &self,
        Parameters(request): Parameters<CropImageRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::crop_image(&self.storage, Parameters(request)).await
    }

    #[tool(description = "OCR文字提取（支持URL列表并发），提取完成后需要使用![](url)是方式展现图片")]
    async fn ocr_extract(
        &self,
        Parameters(request): Parameters<OcrExtractRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::ocr_extract(&self.storage, Parameters(request)).await
    }

    #[tool(
        description = "定位图像中的指定物体，返回二维边界框坐标，在裁剪时，先使用locate_object定位物体，再使用crop_image裁剪物体"
    )]
    async fn locate_object(
        &self,
        Parameters(request): Parameters<LocateObjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::locate_object(Parameters(request)).await
    }

    #[tool(
        description = "AI生成图像，支持 aspect_ratio（1:1、16:9、9:16、4:3、3:4、3:2、2:3）与 resolution（1k、2k、4k），调用前提醒用户可能耗时较长，使用![](url)是方式展现图片"
    )]
    async fn generate_image(
        &self,
        Parameters(request): Parameters<GenerateImageRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::generate_image(&self.storage, Parameters(request)).await
    }

    #[tool(description = "AI编辑图像，使用![](url)是方式展现图片，调用前提醒用户可能耗时较长")]
    async fn edit_image(
        &self,
        Parameters(request): Parameters<EditImageRequest>,
    ) -> Result<CallToolResult, McpError> {
        crate::tools::edit_image(&self.storage, Parameters(request)).await
    }

    // #[tool(description = "查看AI生成/编辑图片历史记录")]
    // async fn list_ai_images(
    //     &self,
    //     Parameters(request): Parameters<ListAiImagesRequest>,
    // ) -> Result<CallToolResult, McpError> {
    //     crate::tools::list_ai_images(&self.storage, Parameters(request)).await
    // }
}

#[tool_handler]
impl ServerHandler for ImageEditorServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
