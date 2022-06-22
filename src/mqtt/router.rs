use async_trait::async_trait;

#[derive(Default)]
pub struct Router {
    table: std::collections::HashMap<String, Box<dyn RouteHandler>>,
}

impl Router {
    pub fn add_handler(&mut self, topic: &str, handler: Box<dyn RouteHandler>) {
        self.table.insert(String::from(topic), handler);
    }

    pub async fn handle_message(&mut self, topic: String, content: &[u8]) -> anyhow::Result<bool> {
        if let Some(handler) = self.table.get_mut(&topic) {
            handler.call(&topic, content).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[async_trait]
pub trait RouteHandler: Send {
    async fn call(&mut self, topic: &str, content: &[u8]) -> anyhow::Result<()>;
}
