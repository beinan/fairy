use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use aws_sdk_s3::error::SdkError;

pub async fn create_s3_client() -> Client{
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    Client::new(&config)
}

pub async fn list_objects(client: &aws_sdk_s3::Client, bucket_name: &str) -> Result<(), Box<dyn std::error::Error>> {

    let response =
        client.list_objects_v2().bucket(bucket_name).send().await.expect("List");

    if let Some(objects) = response.contents {
        for object in objects {
            let key = object.key.expect("Object key not found");
            let size = object.size;

            println!("Object: {} (Size: {} bytes)", key, size);
        }
    } else {
        println!("No objects found in the bucket.");
    }
    Ok(())
}