#[cfg(feature = "direct-methods")]
use crate::message::DirectMethodResponse;
use crate::message::Message;
#[cfg(any(
    feature = "direct-methods",
    feature = "c2d-messages",
    feature = "twin-properties"
))]
use crate::message::MessageType;
use crate::{token::TokenSource, transport::Transport};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use hyper::{client::HttpConnector, header, Body, Client, Request};
use hyper_tls::HttpsConnector;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub(crate) struct HttpsTransport {
    token_source: Box<Arc<dyn TokenSource + Send + Sync + 'static>>,
    hub_name: String,
    device_id: String,
    client: Client<HttpsConnector<HttpConnector>>,
    ping_join_handle: Option<Arc<JoinHandle<()>>>,
    token: String,
    token_expiration: Option<DateTime<Utc>>,
}

impl HttpsTransport {
    pub(crate) async fn new<TS>(
        hub_name: &str,
        device_id: String,
        token_source: TS,
    ) -> crate::Result<Self>
    where
        TS: TokenSource + Send + Sync + 'static,
    {
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);
        let transport = Self {
            hub_name: hub_name.to_string(),
            device_id,
            token_source: Box::new(Arc::new(token_source)),
            client,
            ping_join_handle: None,
            token: String::new(),
            token_expiration: None,
        };

        // transport.ping_join_handle = Some(Arc::new(transport.ping_on_secs_interval(15)));

        Ok(transport)
    }

    ///
    // fn ping_on_secs_interval(&self, ping_interval: u8) -> JoinHandle<()> {
    //     let mut ping_interval = time::interval(time::Duration::from_secs(ping_interval.into()));
    //     let mut cloned_self = self.clone();
    //     tokio::spawn(async move {
    //         loop {
    //             ping_interval.tick().await;

    //             let _ = cloned_self.ping().await;
    //         }
    //     })
    // }

    fn get_token(&mut self) -> &str {
        let now = Utc::now();
        // Generate a new auth token if none exists or the existing one will expire soon
        let needs_new_token = self
            .token_expiration
            .map(|e| e - now < chrono::Duration::minutes(5))
            .unwrap_or(true);

        if needs_new_token {
            let token_lifetime = now + Duration::days(1);
            debug!(
                "Generating new auth token that will expire at {}",
                token_lifetime
            );
            self.token = self.token_source.get(&token_lifetime);
            self.token_expiration = Some(token_lifetime);
        }

        &self.token
    }
}

impl Drop for HttpsTransport {
    fn drop(&mut self) {
        // Check to see whether we're the last instance holding the Arc and only abort the ping if so
        if let Some(handle) = self.ping_join_handle.take() {
            if let Ok(handle) = Arc::try_unwrap(handle) {
                handle.abort();
            }
        }
    }
}

#[async_trait]
impl Transport for HttpsTransport {
    ///
    async fn send_message(&mut self, message: Message) -> crate::Result<()> {
        let req = Request::post(format!(
            "https://{}/devices/{}/messages/events?api-version=2019-03-30",
            self.hub_name, self.device_id
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, self.get_token())
        .body(Body::from(message.body))
        .unwrap();

        match self.client.request(req).await {
            Ok(res) => {
                debug!("Response: {:?}", res);
                Ok(())
            }
            Err(err) => Err(Box::new(err)),
        }
    }
    ///
    #[cfg(feature = "twin-properties")]
    async fn send_property_update(&mut self, _request_id: &str, _body: &str) -> crate::Result<()> {
        unimplemented!()
    }

    ///
    #[cfg(feature = "twin-properties")]
    async fn request_twin_properties(&mut self, _request_id: &str) -> crate::Result<()> {
        unimplemented!()
    }

    ///
    #[cfg(feature = "direct-methods")]
    async fn respond_to_direct_method(
        &mut self,
        _response: DirectMethodResponse,
    ) -> crate::Result<()> {
        unimplemented!()
    }

    ///
    async fn ping(&mut self) -> crate::Result<()> {
        unimplemented!()
    }

    ///
    #[cfg(any(
        feature = "direct-methods",
        feature = "c2d-messages",
        feature = "twin-properties"
    ))]
    async fn get_receiver(&mut self) -> Receiver<MessageType> {
        unimplemented!()
    }
}
