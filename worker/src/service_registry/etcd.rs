// service_registry.rs

use std::sync::{Arc, RwLock};

use etcd_client::{Client, GetOptions, PutOptions};
use log::{debug, error, info};
use thiserror::Error;
use tokio::time;

use crate::settings::SETTINGS;

#[derive(Error, Debug)]
pub enum ServiceRegistryError {
    #[error("etcd error: {0}")]
    EtcdError(#[from] etcd_client::Error),
}

pub struct ServiceRegistry {
    client: Client,
    shared_data: Arc<RwLock<Vec<String>>>,
}

impl ServiceRegistry {
    pub async fn new(etcd_endpoints: &Vec<String>) -> Result<Self, ServiceRegistryError> {
        let client = Client::connect(etcd_endpoints, None).await?;
        let shared_data = Arc::new(RwLock::new(Vec::new()));

        Ok(Self {
            client,
            shared_data,
        })
    }

    pub async fn run(&self) -> Result<(), ServiceRegistryError> {
        let shared_data_clone = Arc::clone(&self.shared_data);
        let shared_data_clone2 = Arc::clone(&self.shared_data);
        let mut client_clone = self.client.clone();
        tokio::spawn(async move {
            let lease_id = match ServiceRegistry::register_service(
                &mut client_clone,
                &SETTINGS.local_ip,
                SETTINGS.http_port,
            )
            .await
            {
                Ok(lease_id) => {
                    info!("Service registered with lease id: {}", lease_id);
                    lease_id
                }
                Err(err) => {
                    error!("Failed to register: {}", err);
                    panic!("Failed to register")
                }
            };

            loop {
                if let Err(err) =
                    ServiceRegistry::update_shared_data(&mut client_clone, &shared_data_clone).await
                {
                    error!("Failed to retrieve services: {}", err);
                }
                if let Err(err) = ServiceRegistry::keep_alive(&mut client_clone, lease_id).await {
                    error!("Failed to keep-alive: {}", err);
                    //todo: retry and panic?
                }
                time::sleep(time::Duration::from_secs(30)).await;
            }
        });

        tokio::spawn(async move {
            loop {
                time::sleep(time::Duration::from_secs(5)).await;
                let data = shared_data_clone2.read().unwrap();
                info!("Registered services: {:?}", *data);
            }
        });

        tokio::task::yield_now().await;

        Ok(())
    }

    async fn update_shared_data(
        client: &mut Client,
        shared_data: &Arc<RwLock<Vec<String>>>,
    ) -> Result<(), ServiceRegistryError> {
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

    async fn register_service(
        client: &mut Client,
        service_host: &String,
        service_port: u16,
    ) -> Result<i64, ServiceRegistryError> {
        // Key and value for the service registration
        let key = format!("services/{}:{}", service_host, service_port);
        let value = format!("{}:{}", service_host, service_port);
        // Register the service in etcd
        let lease_id = client.lease_grant(40, None).await?.id();
        client
            .put(
                key.as_bytes().to_vec(),
                value.as_bytes().to_vec(),
                Some(PutOptions::new().with_lease(lease_id)),
            )
            .await?;

        info!(
            "Registered service with ID: {}:{}, lease ID: {}",
            service_host, service_port, lease_id
        );

        Ok(lease_id)
    }

    async fn keep_alive(client: &mut Client, lease_id: i64) -> Result<(), ServiceRegistryError> {
        let keep_alive_result = client.lease_keep_alive(lease_id).await;
        match keep_alive_result {
            Ok((keeper, _)) => {
                debug!("Lease {} is still alive", keeper.id());
            }
            Err(err) => {
                error!("Failed to keep lease alive: {}", err);
                //todo: re-register the service with a different lease id?
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etcd_error() {
        let err = ServiceRegistryError::EtcdError(etcd_client::Error::InvalidArgs("0".to_string()));
        assert_eq!(format!("{}", err), "etcd error: invalid arguments: 0");
    }
}
