use anyhow::Ok;
use reqwest::Client;
use serde_json::json;

// Query to use in request
const QUERY: &str = "
query ($id: Int) { # Define which variables will be used in the query (id)
  Media (id: $id, type: ANIME) { # Insert our variables into the query arguments (id) (type: ANIME is hard-coded in the query)
    id
    title {
      romaji
      english
      native
    }
  }
}
";

pub async fn get_anilist_data(query: &str) -> anyhow::Result<()> {
    let client = Client::new();
    // Define query and variables
    let json = json!({"query": query, "variables": {"id": 15125}});
    // Make HTTP post request
    let resp = client
        .post("https://graphql.anilist.co/")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string())
        .send()
        .await
        .unwrap()
        .text()
        .await;
    // Get json
    let result: serde_json::Value = serde_json::from_str(&resp.unwrap()).unwrap();
    println!("{:#}", result);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_anilist() {
        let res = get_anilist_data(QUERY).await;
        assert!(res.is_ok())
    }
}
