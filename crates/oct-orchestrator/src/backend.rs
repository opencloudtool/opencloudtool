use std::fs;

use oct_cloud::aws::resource::S3Bucket;
use oct_cloud::resource::Resource;

use crate::config;

/// Creates a state backend based on the configuration.
///
/// Returns a boxed trait object implementing the `StateBackend<T>` trait.
pub(crate) fn get_state_backend<T>(
    state_backend_config: &config::StateBackend,
) -> Box<dyn StateBackend<T>>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + Default + 'static,
{
    log::info!("Using state backend: {state_backend_config:?}");

    match state_backend_config {
        config::StateBackend::Local { path } => Box::new(LocalStateBackend::new(path)),
        config::StateBackend::S3 {
            region,
            bucket,
            key,
        } => Box::new(S3StateBackend::new(region, bucket, key)),
    }
}

#[async_trait::async_trait]
pub(crate) trait StateBackend<T: 'static> {
    /// Saves state to a backend
    async fn save(&self, state: &T) -> Result<(), Box<dyn std::error::Error>>;

    /// Loads state from a backend or initialize a new one
    /// Also returns whether the state was loaded as a boolean
    async fn load(&self) -> Result<(T, bool), Box<dyn std::error::Error>>;

    /// Removes state file from a backend
    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>>;
}

pub(crate) struct LocalStateBackend<T> {
    _marker: std::marker::PhantomData<T>,

    file_path: String,
}

impl<T> LocalStateBackend<T> {
    pub(crate) fn new(file_path: &str) -> Self {
        LocalStateBackend {
            _marker: std::marker::PhantomData,

            file_path: file_path.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl<T> StateBackend<T> for LocalStateBackend<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + Default + 'static,
{
    async fn save(&self, state: &T) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&self.file_path, serde_json::to_string_pretty(state)?)?;

        Ok(())
    }

    async fn load(&self) -> Result<(T, bool), Box<dyn std::error::Error>> {
        if std::path::Path::new(&self.file_path).exists() {
            let existing_data = fs::read_to_string(&self.file_path)?;
            let state = serde_json::from_str::<T>(&existing_data)?;

            Ok((state, true))
        } else {
            Ok((T::default(), false))
        }
    }

    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::remove_file(&self.file_path)?;

        Ok(())
    }
}

#[allow(dead_code)]
pub(crate) struct S3StateBackend<T> {
    _marker: std::marker::PhantomData<T>,

    region: String,
    bucket: String,
    key: String,
}

impl<T> S3StateBackend<T> {
    pub(crate) fn new(region: &str, bucket: &str, key: &str) -> Self {
        S3StateBackend {
            _marker: std::marker::PhantomData,

            region: region.to_string(),
            bucket: bucket.to_string(),
            key: key.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl<T> StateBackend<T> for S3StateBackend<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + Default + 'static,
{
    async fn save(&self, state: &T) -> Result<(), Box<dyn std::error::Error>> {
        let mut s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;
        s3_bucket.create().await?;

        s3_bucket
            .put_object(&self.key, serde_json::to_vec(state)?)
            .await?;

        Ok(())
    }

    async fn load(&self) -> Result<(T, bool), Box<dyn std::error::Error>> {
        let s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;

        let data = s3_bucket.get_object(&self.key).await;

        match data {
            Ok(data) => Ok((serde_json::from_slice(&data)?, true)),
            Err(_) => Ok((T::default(), false)),
        }
    }

    async fn remove(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut s3_bucket = S3Bucket::new(self.region.clone(), self.bucket.clone()).await;

        // For now we expect to have only one file in the bucket
        // If there are multiple files, the state is corrupted and bucket
        // will not be deleted
        s3_bucket.delete_object(&self.key).await?;

        s3_bucket.destroy().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
    struct TestState {
        value: String,
    }

    #[tokio::test]
    async fn test_state_new_exists() {
        // Arrange
        let state_file_content = r#"
{
    "value": "test"
}"#;

        let mut state_file = tempfile::NamedTempFile::new().expect("Failed to create a temp file");
        state_file
            .write_all(state_file_content.as_bytes())
            .expect("Failed to write to file");

        let state_file_path = state_file
            .path()
            .to_str()
            .expect("Failed to convert path to str");
        let state_backend = LocalStateBackend::<TestState>::new(state_file_path);

        // Act
        let (state, loaded) = state_backend
            .load()
            .await
            .expect("Failed to load from state backend");

        // Assert
        assert!(loaded);
        assert_eq!(
            state,
            TestState {
                value: "test".to_string(),
            },
        );
    }

    #[tokio::test]
    async fn test_state_new_not_exists() {
        // Arrange
        let state_backend = LocalStateBackend::<TestState>::new("NO_FILE");

        // Act
        let (state, loaded) = state_backend
            .load()
            .await
            .expect("Failed to load from state backend");

        // Assert
        assert_eq!(state.value, "");
        assert!(!loaded);
    }

    #[tokio::test]
    async fn test_local_state_backend_save() {
        // Arrange
        let state = TestState {
            value: "test".to_string(),
        };

        let state_file = tempfile::NamedTempFile::new().expect("Failed to create a temp file");
        let state_file_path = state_file
            .path()
            .to_str()
            .expect("Failed to convert path to str");
        let state_backend = LocalStateBackend::<TestState>::new(state_file_path);

        // Act
        state_backend
            .save(&state)
            .await
            .expect("Failed to save to state file");

        // Assert
        let file_content = fs::read_to_string(state_file_path).expect("Failed to read from file");

        assert_eq!(
            file_content,
            r#"{
  "value": "test"
}"#
        );
    }

    #[test]
    fn test_s3_backend_new() {
        let state_backend = S3StateBackend::<TestState>::new("region", "bucket", "key");

        assert_eq!(state_backend.region, "region");
        assert_eq!(state_backend.bucket, "bucket");
    }

    #[tokio::test]
    #[ignore = "Requires AWS setup"]
    async fn test_s3_backend_save() {
        let state_backend = S3StateBackend::<TestState>::new("region", "bucket", "key");

        let state = TestState::default();

        state_backend
            .save(&state)
            .await
            .expect("Failed to save to state file");
    }

    #[tokio::test]
    #[ignore = "Requires AWS setup"]
    async fn test_s3_backend_load() {
        let state_backend = S3StateBackend::<TestState>::new("region", "bucket", "key");

        let _ = state_backend
            .load()
            .await
            .expect("Failed to load from state backend");
    }
}
