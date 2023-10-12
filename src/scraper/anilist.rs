use anyhow::{Context, Ok};
use reqwest::Client;
use serde_json::json;

// Query to use in request
//const QUERY: &str = "
//query ($id: Int) { # Define which variables will be used in the query (id)
//  Media (id: $id, type: ANIME) { # Insert our variables into the query arguments (id) (type: ANIME is hard-coded in the query)
//    id
//    title {
//      romaji
//      english
//      native
//    }
//    bannerImage
//    coverImage {
//        medium
//        large
//        extraLarge
//    }
//  }
//}
//";
// bannerImage - wide image

const SEASONAL: &str = "
query ($season: MediaSeason, $seasonYear: Int){
  Page {
    media (season: $season, seasonYear: $seasonYear, type: ANIME){
      id
      title {
        romaji
        english
        native
      }
      description
      episodes
      duration
      averageScore
      meanScore
      popularity
      genres
      coverImage {
          medium
          large
          extraLarge
      }
      startDate {
          year
          month
          day
      }
    }

  }
}
";

#[derive(serde::Deserialize, Debug, Clone)]
pub struct AniShow {
    pub id: Option<u32>,
    pub title: Option<Title>,
    #[serde(rename = "averageScore")]
    pub average_score: Option<u8>,
    #[serde(rename = "meanScore")]
    pub mean_score: Option<u8>,
    pub popularity: Option<u64>,
    pub genres: Option<Vec<String>>,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<CoverImage>,
    pub description: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<FuzzyDate>,
    pub episodes: Option<u16>,
    pub duration: Option<u16>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct FuzzyDate {
    year: Option<u16>,
    month: Option<u8>,
    day: Option<u8>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct CoverImage {
    pub medium: Option<String>,
    pub large: Option<String>,
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
struct MediaPage {
    media: Vec<AniShow>,
}

#[derive(serde::Deserialize, Debug)]
struct Data {
    #[serde(rename = "Page")]
    page: MediaPage,
}

#[derive(serde::Deserialize, Debug)]
struct Response {
    data: Data,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum Season {
    #[serde(rename = "SPRING")]
    SPRING,
    #[serde(rename = "FALL")]
    FALL,
    #[serde(rename = "WINTER")]
    WINTER,
    #[serde(rename = "SUMMER")]
    SUMMER,
}

pub async fn get_anilist_data(
    season: Season,
    year: u16,
) -> anyhow::Result<Vec<AniShow>> {
    let client = Client::new();
    // Define query and variables
    //let json = json!({"query": query, "variables": {"id": 15125}});
    let json = json!({"query": SEASONAL, "variables": {"season": season, "seasonYear": year}});
    let resp = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .await
        .context("Failed to send request")?
        .text()
        .await
        .context("Failed to convert response to text")?;
    let result: Response =
        serde_json::from_str(&resp).context("Failed to deserialize to struct")?;

    Ok(result.data.page.media)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_anilist() {
        match get_anilist_data(Season::FALL, 2023).await {
            core::result::Result::Ok(res) => {
                println!("Success {:?}", res)
            }
            Err(err) => {
                println!("Error: {:?}", err);
                assert!(false)
            }
        };
    }
}
