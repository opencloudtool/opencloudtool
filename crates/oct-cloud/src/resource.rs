/// Defines the basic operations for managing cloud resources.
/// It includes methods for creating and destroying resources asynchronously.
/// Implementations of this trait should provide the specific logic for
/// resource management in the context of the cloud provider being used.
pub trait Resource {
    fn create(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
    fn destroy(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send;
}
