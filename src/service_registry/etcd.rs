// service_registry.rs

use etcd_client::{Client, GetOptions, PutOptions};
use std::error::Error;
use std::sync::{Arc, RwLock};
use tokio::time;
pub struct ServiceRegistry {
    client: Client,
    shared_data: Arc<RwLock<Vec<String>>>,
}

impl ServiceRegistry {
    pub async fn new(etcd_endpoints: [&str; 1]) -> Result<Self, Box<dyn Error>> {
        let client = Client::connect(etcd_endpoints, None).await?;
        let shared_data = Arc::new(RwLock::new(Vec::new()));

        Ok(Self {
            client: client,
            shared_data,
        })
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let shared_data_clone = Arc::clone(&self.shared_data);
        let shared_data_clone2 = Arc::clone(&self.shared_data);
        let mut client_clone = self.client.clone();
        tokio::spawn(async move {
            let lease_id = match ServiceRegistry::register_service(&mut client_clone, rand::random()).await {
                Ok(lease_id) => {
                    println!("Service registered with lease id: {}", lease_id);
                    lease_id
                }
                Err(err) => {
                    panic!("Failed to register: {}", err);
                }
            };

            loop {
                if let Err(err) = ServiceRegistry::update_shared_data(&mut client_clone, &shared_data_clone).await {
                    println!("Failed to retrieve services: {}", err);
                }
                if let Err(err) = ServiceRegistry::keep_alive(&mut client_clone, lease_id).await {
                    println!("Failed to keep-alive: {}", err);
                    //todo: retry and panic?
                }
                time::sleep(time::Duration::from_secs(30)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                time::sleep(time::Duration::from_secs(5)).await;
                let data = shared_data_clone2.read().unwrap();
                println!("Registered services: {:?}", *data);
            }
        });

        tokio::task::yield_now().await;

        Ok(())
    }

    async fn update_shared_data(client: &mut Client, shared_data: &Arc<RwLock<Vec<String>>>) -> Result<(), Box<dyn Error>> {
        let prefix = "services/";
        let options = GetOptions::new().with_prefix();

        let response = client.get(prefix, Some(options)).await?;

        let services: Vec<String> = response
            .kvs()
            .iter()
            .filter_map(|kv| {
                let key_str = kv.key_str().ok()?;
                let service_id = key_str.strip_prefix(prefix)?;

                Some(service_id.to_string())
            })
            .collect();

        let mut data = shared_data.write().unwrap();
        *data = services;

        Ok(())
    }

    async fn register_service(client: &mut Client, service_id: u64) -> Result<i64, Box<dyn Error>> {
        // Key and value for the service registration
        let key = format!("services/{}", service_id);
        let value = "127.0.0.1:8080"; // Replace with actual service address
        
        // Register the service in etcd
        let lease_id = client.lease_grant(40, None).await?.id();
        client.put(key.as_bytes().to_vec(), value.as_bytes().to_vec(), Some(PutOptions::new().with_lease(lease_id))).await?;
    
        println!("Registered service with ID: {}, lease ID: {}", service_id, lease_id);

        Ok(lease_id)
    }
    
    async fn keep_alive(client: &mut Client, lease_id: i64) -> Result<(), Box<dyn Error>> {
        let keep_alive_result = client.lease_keep_alive(lease_id).await;
        match keep_alive_result {
            Ok((keeper, _)) => {
                println!("Lease {} is still alive", keeper.id());
            }
            Err(err) => {
                println!("Failed to keep lease alive: {}", err);
                //todo: re-register the service with a different lease id?
            }
        };
        Ok(())
    }   
}
