#[derive(Default)]
pub struct Generic {
    state: Option<super::StoredProperties>,
}

impl super::Storage for Generic {
    type Error = std::convert::Infallible;

    async fn load(&self) -> Result<Option<super::StoredProperties>, Self::Error> {
        Ok(self.state.clone())
    }

    async fn store(&mut self, properties: super::StoredProperties) -> Result<(), Self::Error> {
        self.state = Some(properties);
        Ok(())
    }
}
