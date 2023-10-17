use std::fmt;

use reqwest::Client;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::json;

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
      studios (isMain: true) {
          nodes {
              name
          }
      }
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

#[derive(Deserialize, Debug, Clone)]
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
    pub studios: Option<Studio>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Studio {
    pub nodes: Option<Vec<Node>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzyDate {
    pub year: Option<u16>,
    #[serde(deserialize_with = "deserialize_month")]
    pub month: Option<String>,
    pub day: Option<u8>,
}

impl fmt::Display for FuzzyDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.year, &self.month, &self.day) {
            (Some(year), Some(month), Some(day)) => write!(f, "{} {}, {}", month, day, year),
            (Some(year), Some(month), None) => write!(f, "{} {}", month, year),
            (Some(year), None, None) => write!(f, "{}", year),
            (None, Some(month), Some(day)) => write!(f, "{} {}", month, day),
            (_, Some(month), None) => write!(f, "{}", month),
            (None, None, Some(day)) => write!(f, "{}", day),
            (None, None, None) => write!(f, ""),
            (Some(_), None, Some(_)) => write!(f, ""),
        }
    }
}

fn deserialize_month<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let month_number = Option::<u8>::deserialize(deserializer)?;
    let month_str = match month_number {
        Some(1) => Some("Jan".to_string()),
        Some(2) => Some("Feb".to_string()),
        Some(3) => Some("Mar".to_string()),
        Some(4) => Some("Apr".to_string()),
        Some(5) => Some("May".to_string()),
        Some(6) => Some("Jun".to_string()),
        Some(7) => Some("Jul".to_string()),
        Some(8) => Some("Aug".to_string()),
        Some(9) => Some("Sep".to_string()),
        Some(10) => Some("Oct".to_string()),
        Some(11) => Some("Nov".to_string()),
        Some(12) => Some("Dec".to_string()),
        Some(_) => return Err(de::Error::custom("Invalid month")),
        None => None,
    };
    Ok(month_str)
}

#[derive(Deserialize, Debug, Clone)]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CoverImage {
    pub medium: Option<String>,
    pub large: Option<String>,
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
}

#[derive(Deserialize, Debug)]
struct MediaPage {
    media: Vec<AniShow>,
}

#[derive(Deserialize, Debug)]
struct Data {
    #[serde(rename = "Page")]
    page: MediaPage,
}

#[derive(Deserialize, Debug)]
struct Response {
    data: Data,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
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

pub async fn get_anilist_data(season: Season, year: u16) -> anyhow::Result<Vec<AniShow>> {
    let client = Client::new();
    // Define query and variables
    let json = json!({"query": SEASONAL, "variables": {"season": season, "seasonYear": year}});
    let resp = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .await?;
    let text_resp = resp.text().await?;
    let result: Response = serde_json::from_str(&text_resp)?;

    Ok(result.data.page.media)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_anilist() {
        match get_anilist_data(Season::FALL, 2023).await {
            core::result::Result::Ok(res) => {
                println!("Success {:?}", res);
            }
            Err(err) => {
                println!("Error: {:?}", err);
                assert!(false)
            }
        };
    }
}
