use crate::configuration::AppConfig;
use crate::container;
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
    pub fn get_registry_name(&self) -> String {
        self.api_base_url
            .strip_prefix("https://")
            .unwrap_or(&self.api_base_url)
            .to_string()
    }

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

    async fn graphql_client(&mut self, query: String) -> Result<reqwest::Response> {
        let query = query
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
        Ok(response)
    }

    pub async fn get_image_list(&mut self) -> Result<Vec<container::Image>> {
        let query = r"query=
{
  RepoListWithNewestImage(requestedPage: {limit: 0, offset:0, sortBy: ALPHABETIC_ASC}) {
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
        .to_string();
        let response = self.graphql_client(query).await?;
        let body: RepoListWithNewestImageResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let list = body.data.RepoListWithNewestImage.Results;
        println!("List: {list:#?}");
        let mut images = Vec::new();
        for result in list {
            let Ok(image) = container::Image::try_from(&result) else {
                println!("Error: skipping invalid image");
                continue;
            };
            images.push(image);
        }
        Ok(images)
    }

    pub async fn get_image_info(&mut self, image_id: String) -> Result<container::Image> {
        let query = format!(
            r#"query=
{{
  Image(image: "{image_id}") {{
    RepoName
    Tag
    LastUpdated
    Digest
    Size
    Title
    Description
  }}
}}
"#
        );
        let response = self.graphql_client(query).await?;

        let body: ImageResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let Some(data) = body.data else {
            // Not found
            println!("Image not found on registry");
            return Err(ClientError::NotFound("Image not found".to_string()).err());
        };
        println!("{data:#?}");
        let image = container::Image::try_from(&data.Image)?;
        println!("Image: {image:#?}");
        Ok(image)
    }

    /// May return an empty list
    pub async fn get_repo_images(&mut self, repo: String) -> Result<Vec<container::Image>> {
        let query = format!(
            r#"query=
{{
  ImageList (repo: "{repo}") {{
    Results {{
      Tag
      RepoName
      Digest
      LastUpdated
      Size
      DownloadCount
    }}
  }}
}}
"#
        );
        let response = self.graphql_client(query).await?;

        let body: ImageListResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let mut images = Vec::new();
        let data = body.data.ImageList.Results;
        println!("Repo {repo} : {data:#?}");
        for image in data {
            images.push(container::Image::try_from(&image)?);
        }
        Ok(images)
    }

    /// May return an empty list
    pub async fn get_digest_images(&mut self, digest: String) -> Result<Vec<container::Image>> {
        let query = format!(
            r#"query=
{{
  ImageListForDigest (id: "{digest}") {{
    Results {{
      Tag
      RepoName
      Digest
      LastUpdated
      Size
      DownloadCount
    }}
  }}
}}
"#
        );
        let response = self.graphql_client(query).await?;

        let body: DigestImageListResponse = match response.json().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                return Err(Error::Program("Can't decode Zot response".to_string()));
            }
        };
        let mut images = Vec::new();
        let data = body.data.ImageListForDigest.Results;
        println!("Digest {digest} : {data:#?}");
        for image in data {
            images.push(container::Image::try_from(&image)?);
        }
        Ok(images)
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

impl TryFrom<&RepoListImage> for container::Image {
    type Error = Error;

    fn try_from(image: &RepoListImage) -> Result<Self> {
        let repository_info = Some(container::RepositoryInfo {
            repository: image.Name.clone(),
            tag: image.NewestImage.Tag.clone(),
        });

        Ok(container::Image {
            repository_info,
            size_bytes: Some(image.Size),
            id: image.NewestImage.Digest.clone(),
            digests: vec![image.NewestImage.Digest.clone()],
            date: image.LastUpdated,
        })
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageResponse {
    pub data: Option<ImageHolder>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageHolder {
    pub Image: ImageInfo, // TODO not RepoListImage
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageInfo {
    pub Description: String,
    pub Title: String, // OCI title, HR
    pub LastUpdated: DateTime<Utc>,
    pub RepoName: String,
    pub Digest: String,
    pub Tag: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub Size: i64,
}

impl TryFrom<&ImageInfo> for container::Image {
    type Error = Error;

    fn try_from(image: &ImageInfo) -> Result<Self> {
        let repository_info = Some(container::RepositoryInfo {
            repository: image.RepoName.clone(),
            tag: image.Tag.clone(),
        });

        Ok(container::Image {
            repository_info,
            size_bytes: Some(image.Size),
            id: image.Digest.clone(),
            digests: vec![image.Digest.clone()],
            date: image.LastUpdated,
        })
    }
}

// TODO refactor.
#[derive(Debug, Clone, Default)]
pub struct RemoteImageInfo {
    pub is_in_remote: bool,
    pub image_info: Option<container::Image>,
}

impl container::Image {
    pub async fn get_remote_info(self, client: &mut Client) -> Result<RemoteImageInfo> {
        println!("checking on remote: {self:#?}");
        let mut image_name = self.get_image_name();
        // Strip domain from repo name
        let mut parts = image_name.split('/');
        let part_count = parts.clone().collect::<Vec<_>>().len();

        if part_count > 1
            && let Some(first) = parts.next()
        {
            // Is domain (crude)
            if first.contains(".") || first.contains("localhost") {
                let mut remainder = String::new();
                for part in parts {
                    remainder += &format!("{part}/");
                }
                remainder.pop(); // remove trailing '/'
                image_name = remainder;
            }
        }
        println!("parsed name: {image_name:?}");

        match client.get_image_info(image_name).await {
            Ok(x) => Ok(RemoteImageInfo {
                is_in_remote: true,
                image_info: Some(x),
            }),
            Err(x) => {
                if x.is_not_found() {
                    Ok(RemoteImageInfo {
                        is_in_remote: false,
                        ..Default::default()
                    })
                } else {
                    Err(x)
                }
            }
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageListResponse {
    pub data: ImageListHolder,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageListHolder {
    pub ImageList: ImageList,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageList {
    pub Results: Vec<ImageListImage>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageListImage {
    pub Tag: String,
    pub RepoName: String,
    pub Digest: String,
    pub LastUpdated: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub Size: i64,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub DownloadCount: i64,
}

impl TryFrom<&ImageListImage> for container::Image {
    type Error = Error;

    fn try_from(image: &ImageListImage) -> Result<Self> {
        let repository_info = Some(container::RepositoryInfo {
            repository: image.RepoName.clone(),
            tag: image.Tag.clone(),
        });

        Ok(container::Image {
            repository_info,
            size_bytes: Some(image.Size),
            id: image.Digest.clone(),
            digests: vec![image.Digest.clone()],
            date: image.LastUpdated,
        })
    }
}

impl container::ImageCollection {
    // Get all remote images that share repo names with the ImageCollection ones
    pub async fn get_images_in_remote(self, client: &mut Client) -> Result<Vec<container::Image>> {
        let local_image = self;
        let mut repos = Vec::new();
        for image_repo_info in local_image.image_aliases {
            let repo = image_repo_info.get_stripped_repository();
            if repo.contains("localhost") {
                continue;
            }
            repos.push(repo);
        }
        repos.sort();
        repos.dedup();
        let mut image_aliases = Vec::new();
        for repo in repos {
            println!("Querying images in repo {repo}");
            let repo_images = client.get_repo_images(repo).await?;
            for image in repo_images {
                image_aliases.push(image);
            }
        }
        Ok(image_aliases)
    }

    // Helper to the previous method
    pub async fn get_full_image_aliases_in_remote(
        self,
        client: &mut Client,
    ) -> Result<Vec<String>> {
        let images = self.get_images_in_remote(client).await?;
        let mut aliases = Vec::new();
        for image in images {
            let alias = format!("{}/{}", client.get_registry_name(), image.get_image_name());
            aliases.push(alias);
        }
        aliases.sort();
        aliases.dedup();
        Ok(aliases)
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DigestImageListResponse {
    pub data: DigestImageListHolder,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DigestImageListHolder {
    pub ImageListForDigest: ImageList,
}
