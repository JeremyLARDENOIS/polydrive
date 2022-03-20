use crate::grpc::file_manager_service_client::FileManagerServiceClient;
use crate::grpc::{File, FileEventType, FileRequest, FileResponse};
use crate::watcher::WatcherListener;
use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use notify::DebouncedEvent;
use std::ffi::OsStr;
use std::path::Path;
use tonic::transport::Channel;

/// The `Indexer` is responsible to handle events on files
/// and to synchronize those files onto the server.
#[derive(Clone)]
pub struct Indexer {
    /// The file manager gRPC client
    client: FileManagerServiceClient<Channel>,
}

impl Indexer {
    /// Bootstrap the server
    pub async fn bootstrap(server_url: String) -> Result<Self> {
        info!("bootstrapping indexer");

        let client = FileManagerServiceClient::connect(server_url).await?;

        Ok(Self { client })
    }

    /// Notify the remote server that a new event was emitted
    async fn notify(&self, data: FileRequest) -> Result<FileResponse> {
        debug!("sending notify request to remote server");
        Ok(self.client.clone().file_event(data).await?.into_inner())
    }

    /// Index a file on the remote server.
    async fn index(&self, path: &Path, event: FileEventType) -> Result<()> {
        info!("indexing new file {}", path.display());
        let _ = std::fs::File::open(path)?;

        let filename = path.file_name().unwrap_or_else(|| OsStr::new("file"));
        let extension = path.extension().unwrap_or_else(|| OsStr::new("txt"));

        debug!(
            "requesting link for event={:?}, filename={:?}, extension={:?}, path={:?}",
            &event, filename, extension, path
        );

        let response = self
            .notify(FileRequest {
                client_name: None,
                event_type: event.into(),
                file: Some(File {
                    path: path.display().to_string(),
                    base_name: filename.to_str().unwrap().to_string(),
                    created: None,
                    last_updated: None,
                }),
            })
            .await?;

        debug!(
            "received pre-signed url for file={:?}, url={:?}",
            filename, &response.link
        );

        // TODO Hugo: upload here

        info!("successfully indexed file {}", path.display());

        Ok(())
    }
}

#[async_trait]
impl WatcherListener for Indexer {
    async fn on_event(&self, event: &DebouncedEvent) -> Result<()> {
        match event {
            DebouncedEvent::Create(path) => {
                debug!("new file detected. file={}", &path.display());
                if let Err(e) = self.index(path.as_path(), FileEventType::from(event)).await {
                    error!(
                        "an error occurred when trying to index the file. details={}",
                        e
                    )
                }
            }
            DebouncedEvent::Write(path) => {
                debug!("modification detected. file={}", &path.display());
                warn!("behavior not implemented");
            }
            DebouncedEvent::Chmod(path) => {
                debug!("file attributes updated. file={}", &path.display());
                warn!("behavior not implemented");
            }
            DebouncedEvent::Remove(path) => {
                debug!("removed file. file={}", &path.display());
                warn!("behavior not implemented");
            }
            DebouncedEvent::Rename(old, new) => {
                debug!(
                    "file renamed. old={}, new={}",
                    &old.display(),
                    &new.display()
                );
                warn!("behavior not implemented");
            }
            DebouncedEvent::Rescan => {
                warn!("a problem has been detected that makes it necessary to re-scan the watched directories.");
                warn!("behavior not implemented");
            }
            DebouncedEvent::Error(e, path) => {
                if let Some(path) = path {
                    error!(
                        "an error occurred on path={}. details={}",
                        &path.display(),
                        e
                    );
                }
                error!("an error occurred. details={}", e);
            }
            _ => {
                debug!("ignoring event = {:?} ", event);
            }
        };

        Ok(())
    }
}