use std::sync::Arc;

use anyhow::Result;

use crate::alerts::{AlertsRaw, load_raw_alerts};
use crate::bus::load_bus;
use crate::cache::{CacheCell, TtlMap};
use crate::config::Config;
use crate::facilities::{FacilityData, load_facilities};
use crate::fares::{RailFareMatrix, load_rail_fares};
use crate::ferries::{FerryData, load_ferries};
use crate::http::HttpBackend;
#[cfg(feature = "native")]
use crate::http::HttpClient;
use crate::light_rail::{
    LightRailData, LightRailFareMatrix, load_light_rail, load_light_rail_fares,
};
use crate::model::{BusData, Network};
use crate::network::load_network;
use crate::status::{
    LegendData, LineConfigMap, RawStatus, load_legend, load_line_config, load_raw_status,
};

#[derive(Clone)]
pub struct App<H: HttpBackend> {
    pub config: Arc<Config>,
    pub http: H,
    network: Arc<CacheCell<Network>>,
    rail_fares: Arc<CacheCell<RailFareMatrix>>,
    light_rail: Arc<CacheCell<LightRailData>>,
    light_rail_fares: Arc<CacheCell<LightRailFareMatrix>>,
    bus: Arc<CacheCell<BusData>>,
    facilities: Arc<CacheCell<FacilityData>>,
    ferries: Arc<CacheCell<FerryData>>,
    alerts: Arc<CacheCell<AlertsRaw>>,
    status: Arc<CacheCell<RawStatus>>,
    legend: Arc<CacheCell<LegendData>>,
    line_config: Arc<CacheCell<LineConfigMap>>,
    journeys: Arc<TtlMap>,
}

#[cfg(feature = "native")]
impl App<HttpClient> {
    pub fn new() -> Result<Self> {
        let config = Config::from_env();
        let http = HttpClient::new(config.timeout, config.user_agent.clone())?;
        Ok(Self::with_http(config, http))
    }
}

impl<H: HttpBackend> App<H> {
    pub fn with_http(config: Config, http: H) -> Self {
        Self {
            network: Arc::new(CacheCell::new(config.cache.static_data)),
            rail_fares: Arc::new(CacheCell::new(config.cache.static_data)),
            light_rail: Arc::new(CacheCell::new(config.cache.static_data)),
            light_rail_fares: Arc::new(CacheCell::new(config.cache.static_data)),
            bus: Arc::new(CacheCell::new(config.cache.static_data)),
            facilities: Arc::new(CacheCell::new(config.cache.static_data)),
            ferries: Arc::new(CacheCell::new(config.cache.static_data)),
            alerts: Arc::new(CacheCell::new(config.cache.alerts)),
            status: Arc::new(CacheCell::new(config.cache.status)),
            legend: Arc::new(CacheCell::new(config.cache.static_data)),
            line_config: Arc::new(CacheCell::new(config.cache.static_data)),
            journeys: Arc::new(TtlMap::new(config.cache.journey)),
            config: Arc::new(config),
            http,
        }
    }

    pub async fn network(&self) -> Result<Arc<Network>> {
        self.network
            .get_or_try_init(|| async { load_network(&self.http, &self.config).await })
            .await
    }

    pub async fn rail_fares(&self) -> Result<Arc<RailFareMatrix>> {
        self.rail_fares
            .get_or_try_init(|| async { load_rail_fares(&self.http, &self.config).await })
            .await
    }

    pub async fn light_rail(&self) -> Result<Arc<LightRailData>> {
        self.light_rail
            .get_or_try_init(|| async { load_light_rail(&self.http, &self.config).await })
            .await
    }

    pub async fn light_rail_fares(&self) -> Result<Arc<LightRailFareMatrix>> {
        self.light_rail_fares
            .get_or_try_init(|| async { load_light_rail_fares(&self.http, &self.config).await })
            .await
    }

    pub async fn bus(&self) -> Result<Arc<BusData>> {
        self.bus
            .get_or_try_init(|| async { load_bus(&self.http, &self.config).await })
            .await
    }

    pub async fn facilities(&self) -> Result<Arc<FacilityData>> {
        self.facilities
            .get_or_try_init(|| async { load_facilities(&self.http, &self.config).await })
            .await
    }

    pub async fn ferries(&self) -> Result<Arc<FerryData>> {
        self.ferries
            .get_or_try_init(|| async { load_ferries(&self.http, &self.config).await })
            .await
    }

    pub async fn alerts(&self) -> Result<Arc<AlertsRaw>> {
        self.alerts
            .get_or_try_init(|| async { load_raw_alerts(&self.http, &self.config).await })
            .await
    }

    pub async fn status(&self) -> Result<Arc<RawStatus>> {
        self.status
            .get_or_try_init(|| async { load_raw_status(&self.http, &self.config).await })
            .await
    }

    pub async fn legend(&self) -> Result<Arc<LegendData>> {
        self.legend
            .get_or_try_init(|| async { load_legend(&self.http, &self.config).await })
            .await
    }

    pub async fn line_config(&self) -> Result<Arc<LineConfigMap>> {
        self.line_config
            .get_or_try_init(|| async { load_line_config(&self.http, &self.config).await })
            .await
    }

    pub fn journeys(&self) -> &TtlMap {
        &self.journeys
    }
}
