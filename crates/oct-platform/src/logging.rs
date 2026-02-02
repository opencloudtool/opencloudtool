use serde::Serialize;
use tokio::sync::broadcast;
use tracing_subscriber::Layer;

pub struct LogLayer {
    pub sender: broadcast::Sender<String>,
}

#[derive(Serialize)]
struct LogMessage {
    timestamp: String,
    level: String,
    message: String,
}

impl<S> Layer<S> for LogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = StringVisitor::default();
        event.record(&mut visitor);

        let log_msg = LogMessage {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: event.metadata().level().to_string(),
            message: visitor.message,
        };

        if let Ok(json) = serde_json::to_string(&log_msg) {
            let _ = self.sender.send(json);
        }
    }
}

#[derive(Default)]
struct StringVisitor {
    message: String,
}

impl tracing::field::Visit for StringVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}").trim_matches('"').to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::prelude::*;

    use super::*;

    #[test]
    fn test_log_layer_broadcasts_message() {
        // Arrange
        let (tx, mut rx) = broadcast::channel(1);
        let layer = LogLayer { sender: tx };

        let subscriber = tracing_subscriber::Registry::default().with(layer);

        // Act
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("test log message");
        });

        // Assert
        let msg = rx.try_recv().expect("Should have received a log message");

        // We can't strictly check the full JSON string because of the timestamp,
        // but we can verify it contains the log level and message.
        assert!(msg.contains("\"level\":\"INFO\""));
        assert!(msg.contains("\"message\":\"test log message\""));
    }
}
