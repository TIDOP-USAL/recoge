use crate::configuration::AppConfig;
use crate::error::{ClientError, Error, RemoteError, Result};
use chrono::{DateTime, Utc};
use reqwest::Method;
use reqwest::RequestBuilder;
use reqwest::Url;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

pub struct Client {
    api_base_url: String,
    username: String,
    password: String,
}

impl Client {
    pub fn new(config: &AppConfig) -> Result<Self> {
        if config.remote_url.is_empty() {
            println!("Can't create zot client without API URL"); // TODO public images support
            return Err(ClientError::Invalid("Missing URL".to_string()).err());
        }
        if config.is_missing_credentials() {
            println!("Can't create zot client without user credentials"); // TODO public images support
            return Err(ClientError::Invalid("Missing credentials".to_string()).err());
        }
        println!("Zot client created");
        let mut api_base_url = config.remote_url.clone();
        if !api_base_url.starts_with("https://") {
            api_base_url = format!("https://{api_base_url}");
        }
        let client = Self {
            api_base_url,
            username: config.user.clone(),
            password: config.password.clone(),
        };
        Ok(client)
    }

    fn client(
        &mut self,
        method: Method,
        path: &str,
        query: Option<&str>,
    ) -> Result<RequestBuilder> {
        let Ok(mut url) = Url::parse(&self.api_base_url) else {
            println!("URL parse error");
            return Err(ClientError::Invalid("URL".to_string()).err());
        };
        url.set_scheme("https").unwrap();
        url.set_path(path);
        url.set_query(query);
        println!("Using URL: {url}");
        let client = reqwest::Client::new().request(method, url.as_str());
        let client = client
            .header(
                "User-Agent",
                format!("recoge v{}", env!("CARGO_PKG_VERSION")),
            )
            .header(ACCEPT, "application/json, text/plain, */*")
            .basic_auth(&self.username, Some(&self.password));
        Ok(client)
    }

    pub async fn get_image_list(&mut self) -> Result<Vec<String>> {
        let query = r"query=
{
  RepoListWithNewestImage(requestedPage: {limit: 0, offset:0, sortBy: ALPHABETIC_ASC}) {
    Page {
      TotalCount
      ItemCount
    }
    Results {
      Name
      LastUpdated
      Size
      Platforms {
        Os
        Arch
      }
      NewestImage {
        Digest
        Tag
      }
    }
  }
}
"
        .split(' ')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        let Ok(response) = self
            .client(Method::GET, "/v2/_zot/ext/search", Some(&query))?
            .send()
            .await
        else {
            return Err(Error::Program("Can't reach zot".to_string()));
        };
        println!("{response:?}");
        if response.status().is_client_error() {
            return Err(Error::Program("zot client err".to_string()));
        }
        if response.status().is_server_error() {
            return Err(RemoteError::Generic("zot err".to_string()).err());
        }

        let body: RepoListWithNewestImageResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let list = body.data.RepoListWithNewestImage.Results;
        println!("List: {list:#?}");
        let mut names = Vec::new();
        for image in list {
            names.push(format!("{}:{}", image.Name, image.NewestImage.Tag));
        }
        Ok(names)
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImageResponse {
    pub data: RepoListWithNewestImageHolder,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImageHolder {
    pub RepoListWithNewestImage: RepoListWithNewestImage,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListWithNewestImage {
    pub Results: Vec<RepoListImage>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoListImage {
    pub Name: String,
    pub LastUpdated: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub Size: i64,
    pub NewestImage: ImageTags,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageTags {
    pub Digest: String,
    pub Tag: String,
}
