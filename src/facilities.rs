use std::collections::HashMap;

use anyhow::Result;

use crate::config::Config;
use crate::csvutil::{parse_csv_objects, text};
use crate::http::HttpClient;
use crate::model::{
    Coordinates, Facility, FacilityGroup, FacilityItem, RailStation, StationFacilities, StationRef,
};

#[derive(Debug, Clone)]
struct Category {
    category: String,
    category_tc: String,
    facility: String,
    facility_tc: String,
    order: i64,
}

#[derive(Debug, Clone)]
struct FacilityRow {
    key: String,
    value: bool,
    exit_en: String,
    exit_tc: String,
    location: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FacilityData {
    by_station: HashMap<i64, Vec<FacilityRow>>,
    categories: HashMap<String, Category>,
}

fn parse_location(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Some(value.to_string());
    }
    if parts.iter().all(|part| part.parse::<f64>().is_ok()) {
        Some(format!("{},{}", parts[0], parts[1]))
    } else {
        Some(value.to_string())
    }
}

pub fn build_facility_data(categories_csv: &str, facilities_csv: &str) -> Result<FacilityData> {
    let mut categories = HashMap::new();
    for row in parse_csv_objects(categories_csv)? {
        let key = text(&row, "Item_Code");
        if key.is_empty() {
            continue;
        }
        categories.insert(
            key.to_string(),
            Category {
                category: text(&row, "Category_En").to_string(),
                category_tc: text(&row, "Category_Zh").to_string(),
                facility: text(&row, "Facility_En").to_string(),
                facility_tc: text(&row, "Facility_Zh").to_string(),
                order: text(&row, "Sorting_Order").parse::<i64>().unwrap_or(0),
            },
        );
    }

    let mut by_station: HashMap<i64, Vec<FacilityRow>> = HashMap::new();
    for row in parse_csv_objects(facilities_csv)? {
        let Ok(station_id) = text(&row, "Station_No").parse::<i64>() else {
            continue;
        };
        by_station.entry(station_id).or_default().push(FacilityRow {
            key: text(&row, "Key").to_string(),
            value: text(&row, "Value").eq_ignore_ascii_case("Y"),
            exit_en: text(&row, "AJTextEn").to_string(),
            exit_tc: text(&row, "AJTextZh").to_string(),
            location: parse_location(text(&row, "Exit_Coordinate_X_Y")),
        });
    }

    Ok(FacilityData {
        by_station,
        categories,
    })
}

pub async fn load_facilities(http: &HttpClient, config: &Config) -> Result<FacilityData> {
    let (categories, facilities) = tokio::join!(
        http.get_text(&config.datasets.barrier_free_categories, &[]),
        http.get_text(&config.datasets.barrier_free, &[]),
    );
    build_facility_data(&categories?, &facilities?)
}

pub fn facilities_for_station(data: &FacilityData, station: &RailStation) -> StationFacilities {
    let rows: Vec<&FacilityRow> = data
        .by_station
        .get(&station.id)
        .map(|rows| rows.iter().filter(|row| row.value).collect())
        .unwrap_or_default();

    let mut groups: Vec<FacilityGroup> = Vec::new();
    for row in rows {
        let meta = data.categories.get(&row.key);
        let category_name = meta
            .map(|item| item.category.clone())
            .unwrap_or_else(|| "Other facilities".to_string());
        let category_tc = meta
            .map(|item| item.category_tc.clone())
            .unwrap_or_else(|| "其他設施".to_string());
        let facility_name = meta
            .map(|item| item.facility.clone())
            .unwrap_or_else(|| row.key.clone());
        let facility_tc = meta
            .map(|item| item.facility_tc.clone())
            .unwrap_or_else(|| row.key.clone());

        let group_index = match groups
            .iter()
            .position(|group| group.category == category_name)
        {
            Some(index) => index,
            None => {
                groups.push(FacilityGroup {
                    category: category_name.clone(),
                    category_tc,
                    facilities: Vec::new(),
                });
                groups.len() - 1
            }
        };

        let facility_index = match groups[group_index]
            .facilities
            .iter()
            .position(|facility| facility.name == facility_name)
        {
            Some(index) => index,
            None => {
                groups[group_index].facilities.push(Facility {
                    name: facility_name,
                    name_tc: facility_tc,
                    items: Vec::new(),
                });
                groups[group_index].facilities.len() - 1
            }
        };

        groups[group_index].facilities[facility_index]
            .items
            .push(FacilityItem {
                key: row.key.clone(),
                exit_en: row.exit_en.clone(),
                exit_tc: row.exit_tc.clone(),
                location: row.location.clone(),
            });
    }

    groups.sort_by(|a, b| {
        let order_a = data
            .categories
            .values()
            .filter(|category| category.category == a.category)
            .map(|category| category.order)
            .min()
            .unwrap_or(99);
        let order_b = data
            .categories
            .values()
            .filter(|category| category.category == b.category)
            .map(|category| category.order)
            .min()
            .unwrap_or(99);
        order_a
            .cmp(&order_b)
            .then_with(|| a.category.cmp(&b.category))
    });

    let note = if groups.is_empty() {
        "No barrier-free facility records were found for this station.".to_string()
    } else {
        "Facilities marked available in MTR's barrier-free dataset. Confirm lifts are in service on the day; planned maintenance is not reflected here.".to_string()
    };

    StationFacilities {
        station: StationRef {
            id: station.id,
            code: station.code.clone(),
            name_en: station.name_en.clone(),
            name_tc: station.name_tc.clone(),
        },
        categories: groups,
        note,
        lines: Vec::new(),
        coordinates: match (station.lat, station.lng) {
            (Some(latitude), Some(longitude)) => Some(Coordinates {
                latitude,
                longitude,
            }),
            _ => None,
        },
    }
}
