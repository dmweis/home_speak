use async_trait::async_trait;
use log::*;
use rumqttc::{AsyncClient, QoS, SubscribeFilter};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("unsupported topic")]
    UnsupportedTopicName { topic: String },
    #[error("subscription error")]
    SubscriptionError(#[from] rumqttc::ClientError),
}

#[derive(Default)]
pub struct Router {
    table: std::collections::HashMap<String, Box<dyn RouteHandler>>,
}

impl Router {
    pub fn add_handler(
        &mut self,
        topic: &str,
        handler: Box<dyn RouteHandler>,
    ) -> std::result::Result<(), RouterError> {
        if topic.contains('#') || topic.contains('+') {
            Err(RouterError::UnsupportedTopicName {
                topic: topic.to_owned(),
            })
        } else {
            self.table.insert(String::from(topic), handler);
            Ok(())
        }
    }

    pub async fn handle_message(&mut self, topic: String, content: &[u8]) -> anyhow::Result<bool> {
        if let Some(handler) = self.table.get_mut(&topic) {
            handler.call(&topic, content).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn subscribe_to_topics(&self, mqtt_client: &AsyncClient) -> Result<(), RouterError> {
        let topics = self.table.keys().map(|topic_name| SubscribeFilter {
            path: topic_name.to_owned(),
            qos: QoS::AtMostOnce,
        });
        mqtt_client.subscribe_many(topics).await?;
        Ok(())
    }
}

#[async_trait]
pub trait RouteHandler: Send + Sync {
    async fn call(&mut self, topic: &str, content: &[u8]) -> anyhow::Result<()>;
}
