use crate::scraper::anilist::{AniShow, Title, CoverImage};

pub fn unwrap_or_na<T: std::fmt::Display>(value: &Option<T>) -> ::askama::Result<String> {
    Ok(value.as_ref().map_or("N/A".to_string(), |v| v.to_string()))
}

pub fn unwrap_title_romaji(title: &Option<Title>) -> ::askama::Result<String> {
    Ok(title
        .as_ref()
        .and_then(|t| t.romaji.as_ref())
        .map_or("N/A".to_string(), |v| v.to_string()))
}

pub fn unwrap_cover(cover: &Option<CoverImage>) -> ::askama::Result<String> {
    Ok(cover
       .as_ref()
       .and_then(|c| c.large.as_ref())
       .map_or("https://upload.wikimedia.org/wikipedia/commons/thumb/6/65/No-Image-Placeholder.svg/800px-No-Image-Placeholder.svg.png".into(), |v| v.to_string())
        )
}

pub fn unwrap_score(show: &AniShow) -> ::askama::Result<String> {
    let average_score = show.average_score;
    let mean_score = show.mean_score;
    match (average_score, mean_score) {
        (Some(val1), Some(val2)) => {
            let max_val = val1.min(val2) as f64 / 10.0;
            Ok(format!("{:.2}", max_val))
        }
        (Some(val1), None) => {
            let val = val1 as f64 / 10.0;
            Ok(format!("{:.2}", val))
        }
        (None, Some(val2)) => {
            let val = val2 as f64 / 10.0;
            Ok(format!("{:.2}", val))
        }
        (None, None) => Ok("N/A".into()),
    }
}

pub fn unwrap_members(show: &AniShow) -> ::askama::Result<String> {
    let members = show.popularity;
    match members {
        Some(members) => {
            if members < 1000 {
                Ok(members.to_string())
            } else {
                let members_val = members / 1000;
                Ok(format!("{:?}K", members_val))
            }
        }
        None => Ok("N/A".into())
    }
}
