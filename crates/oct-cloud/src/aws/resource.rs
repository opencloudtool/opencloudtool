use crate::aws::client::S3;
use crate::resource::Resource;

pub struct S3Bucket {
    client: S3,

    pub region: String,
    pub name: String,
}

impl S3Bucket {
    pub async fn new(region: String, name: String) -> Self {
        // Load AWS configuration
        let region_provider = aws_sdk_s3::config::Region::new(region.clone());
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .credentials_provider(
                aws_config::profile::ProfileFileCredentialsProvider::builder()
                    .profile_name("default")
                    .build(),
            )
            .region(region_provider)
            .load()
            .await;

        let s3_client = aws_sdk_s3::Client::new(&config);

        Self {
            client: S3::new(s3_client),
            region,
            name,
        }
    }

    /// Put an object in the bucket
    pub async fn put_object(
        &self,
        key: &str,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.client.put_object(&self.name, key, data).await?;

        Ok(())
    }

    /// Get an object from the bucket
    pub async fn get_object(&self, key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.client.get_object(&self.name, key).await
    }

    /// Delete an object from the bucket
    pub async fn delete_object(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_object(&self.name, key).await?;

        Ok(())
    }
}

impl Resource for S3Bucket {
    async fn create(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client.create_bucket(&self.region, &self.name).await?;

        Ok(())
    }

    async fn destroy(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.client.delete_bucket(&self.name).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mockall::predicate::eq;

    #[tokio::test]
    async fn test_s3_bucket_create_success() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_create_bucket()
            .with(eq("region".to_string()), eq("bucket".to_string()))
            .return_once(|_, _| Ok(()));

        let mut s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.create().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_s3_bucket_create_failure() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_create_bucket()
            .with(eq("region".to_string()), eq("bucket".to_string()))
            .return_once(|_, _| Err("error".into()));

        let mut s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.create().await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_s3_bucket_destroy_success() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_delete_bucket()
            .with(eq("bucket".to_string()))
            .return_once(|_| Ok(()));

        let mut s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.destroy().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_s3_bucket_destroy_failure() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_delete_bucket()
            .with(eq("bucket".to_string()))
            .return_once(|_| Err("error".into()));

        let mut s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.destroy().await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_s3_bucket_put_object_success() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_put_object()
            .with(
                eq("bucket".to_string()),
                eq("key".to_string()),
                eq("content".as_bytes().to_vec()),
            )
            .return_once(|_, _, _| Ok(()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket
            .put_object("key", "content".as_bytes().to_vec())
            .await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_s3_bucket_put_object_failure() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_put_object()
            .with(
                eq("bucket".to_string()),
                eq("key".to_string()),
                eq("content".as_bytes().to_vec()),
            )
            .return_once(|_, _, _| Err("error".into()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket
            .put_object("key", "content".as_bytes().to_vec())
            .await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_s3_bucket_get_object_success() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_get_object()
            .with(eq("bucket".to_string()), eq("key".to_string()))
            .return_once(|_, _| Ok("content".as_bytes().to_vec()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.get_object("key").await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_s3_bucket_get_object_failure() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_get_object()
            .with(eq("bucket".to_string()), eq("key".to_string()))
            .return_once(|_, _| Err("error".into()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.get_object("key").await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_s3_bucket_delete_object_success() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_delete_object()
            .with(eq("bucket".to_string()), eq("key".to_string()))
            .return_once(|_, _| Ok(()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.delete_object("key").await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_s3_bucket_delete_object_failure() {
        // Arrange
        let mut s3_impl_mock = S3::default();
        s3_impl_mock
            .expect_delete_object()
            .with(eq("bucket".to_string()), eq("key".to_string()))
            .return_once(|_, _| Err("error".into()));

        let s3_bucket = S3Bucket {
            client: s3_impl_mock,
            name: "bucket".to_string(),
            region: "region".to_string(),
        };

        // Act
        let result = s3_bucket.delete_object("key").await;

        // Assert
        assert!(result.is_err());
    }
}
