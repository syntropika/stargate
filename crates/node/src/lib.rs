use napi_derive::napi;
use rust::NativeRuntime;
use std::sync::Arc;
#[napi]
pub struct Native {
    inner: Arc<NativeRuntime>,
}
#[napi]
impl Native {
    #[napi(factory)]
    pub async fn create(configuration: String) -> napi::Result<Self> {
        let inner = tokio::task::spawn_blocking(move || NativeRuntime::create(&configuration))
            .await
            .map_err(|_| napi::Error::from_reason("runtime unavailable"))?
            .map_err(napi::Error::from_reason)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
    #[napi]
    pub async fn handle(&self, request: String) -> napi::Result<String> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.handle(&request))
            .await
            .map_err(|_| napi::Error::from_reason("runtime unavailable"))?
            .map_err(napi::Error::from_reason)
    }
    #[napi]
    pub async fn authorize(&self, input: String) -> napi::Result<String> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || inner.authorize(&input))
            .await
            .map_err(|_| napi::Error::from_reason("runtime unavailable"))?
            .map_err(napi::Error::from_reason)
    }
}
