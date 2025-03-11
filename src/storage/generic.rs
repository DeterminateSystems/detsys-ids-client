use tokio::sync::RwLock;

#[derive(Default)]
pub struct Generic {
    state: RwLock<Option<super::StoredProperties>>,
}

impl super::Storage for Generic {
    type Error = std::convert::Infallible;

    async fn load(&self) -> Result<Option<super::StoredProperties>, Self::Error> {
        Ok((*self.state.read().await).clone())
    }

    async fn store(&mut self, properties: &super::StoredProperties) -> Result<(), Self::Error> {
        *self.state.write().await = Some(properties.clone());
        Ok(())
    }
}
