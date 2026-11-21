use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::csvutil::{num, parse_csv_objects, text};
use crate::http::HttpClient;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OctopusFare {
    pub adult: Option<f64>,
    pub student: Option<f64>,
    pub joy_you_sixty: Option<f64>,
    pub child: Option<f64>,
    pub elderly: Option<f64>,
    pub pwd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleFare {
    pub adult: Option<f64>,
    pub child: Option<f64>,
    pub elderly: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RailFare {
    pub from_id: i64,
    pub to_id: i64,
    pub octopus: OctopusFare,
    pub single: SingleFare,
}

pub type RailFareMatrix = HashMap<String, RailFare>;

pub fn fare_key(from_id: i64, to_id: i64) -> String {
    format!("{from_id}:{to_id}")
}

pub fn build_rail_fare_matrix(csv: &str) -> Result<RailFareMatrix> {
    let mut matrix = HashMap::new();
    for row in parse_csv_objects(csv)? {
        let Ok(from_id) = text(&row, "SRC_STATION_ID").parse::<i64>() else {
            continue;
        };
        let Ok(to_id) = text(&row, "DEST_STATION_ID").parse::<i64>() else {
            continue;
        };

        matrix.insert(
            fare_key(from_id, to_id),
            RailFare {
                from_id,
                to_id,
                octopus: OctopusFare {
                    adult: num(row.get("OCT_ADT_FARE")),
                    student: num(row.get("OCT_STD_FARE")),
                    joy_you_sixty: num(row.get("OCT_JOYYOU_SIXTY_FARE")),
                    child: num(row.get("OCT_CON_CHILD_FARE")),
                    elderly: num(row.get("OCT_CON_ELDERLY_FARE")),
                    pwd: num(row.get("OCT_CON_PWD_FARE")),
                },
                single: SingleFare {
                    adult: num(row.get("SINGLE_ADT_FARE")),
                    child: num(row.get("SINGLE_CON_CHILD_FARE")),
                    elderly: num(row.get("SINGLE_CON_ELDERLY_FARE")),
                },
            },
        );
    }
    Ok(matrix)
}

pub async fn load_rail_fares(http: &HttpClient, config: &Config) -> Result<RailFareMatrix> {
    let csv = http.get_text(&config.datasets.lines_fares, &[]).await?;
    build_rail_fare_matrix(&csv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fare_matrix_keeps_all_categories() {
        let csv = [
            "SRC_STATION_NAME,SRC_STATION_ID,DEST_STATION_NAME,DEST_STATION_ID,OCT_ADT_FARE,OCT_STD_FARE,OCT_JOYYOU_SIXTY_FARE,SINGLE_ADT_FARE,OCT_CON_CHILD_FARE,OCT_CON_ELDERLY_FARE,OCT_CON_PWD_FARE,SINGLE_CON_CHILD_FARE,SINGLE_CON_ELDERLY_FARE",
            "Admiralty,1,Tung Chung,4,24.3,12.5,2.0,28.0,12.5,4.9,2.0,15.0,15.0",
        ].join("\n");
        let matrix = build_rail_fare_matrix(&csv).unwrap();
        let fare = matrix.get(&fare_key(1, 4)).unwrap();
        assert_eq!(fare.octopus.adult, Some(24.3));
        assert_eq!(fare.octopus.student, Some(12.5));
        assert_eq!(fare.single.elderly, Some(15.0));
    }
}
