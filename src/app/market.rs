use std::collections::HashMap;

use gloo_net::http::Request;
use js_sys::JsString;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

pub const MARKET_REGION: &str = "中国";
const UNIVERSALIS_BASE_URL: &str = "https://universalis.app";
const MARKET_DB_NAME: &str = "xiv-companion-market-cache";
const MARKET_STORE_NAME: &str = "quotes";
const MARKET_DB_VERSION: u32 = 1;
const PRICE_TTL_MS: i64 = 30 * 60 * 1_000;
const HISTORY_TTL_MS: i64 = 12 * 60 * 60 * 1_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SalesWindow {
    pub recent: f64,
    pub previous: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MarketQualityQuote {
    pub unit_price: Option<u32>,
    pub basis: Option<String>,
    pub daily_sales: Option<f64>,
    pub sales_window: Option<SalesWindow>,
    aggregated_unit_price: Option<u32>,
    recent_sale_price: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MarketQuote {
    pub region: String,
    pub item_id: u32,
    pub updated_at: Option<i64>,
    pub nq: MarketQualityQuote,
    pub hq: MarketQualityQuote,
    pub price_fetched_at: i64,
    pub history_fetched_at: i64,
}

impl MarketQuote {
    pub fn unit_price(&self) -> Option<u32> {
        self.nq.unit_price.or(self.hq.unit_price)
    }

    pub fn basis(&self) -> Option<&str> {
        self.nq.basis.as_deref().or(self.hq.basis.as_deref())
    }

    pub fn daily_sales(&self) -> Option<f64> {
        sum_optional(self.nq.daily_sales, self.hq.daily_sales)
    }

    pub fn sales_window(&self) -> Option<SalesWindow> {
        combine_windows(self.nq.sales_window, self.hq.sales_window)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarketRefreshResult {
    pub quotes: HashMap<u32, MarketQuote>,
    pub error: Option<String>,
}

#[derive(Clone, Deserialize)]
struct AggregatedResponse {
    #[serde(default)]
    results: Vec<AggregatedItem>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AggregatedItem {
    item_id: u32,
    #[serde(default)]
    nq: AggregatedQuality,
    #[serde(default)]
    hq: AggregatedQuality,
    #[serde(default)]
    world_upload_times: Vec<UploadTime>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AggregatedQuality {
    min_listing: Option<PriceScope>,
    recent_purchase: Option<PriceScope>,
    average_sale_price: Option<PriceScope>,
}

#[derive(Clone, Deserialize)]
struct PriceScope {
    region: Option<PriceValue>,
}

#[derive(Clone, Deserialize)]
struct PriceValue {
    price: Option<f64>,
}

#[derive(Clone, Deserialize)]
struct UploadTime {
    timestamp: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    timestamp: f64,
    quantity: f64,
    price_per_unit: f64,
    #[serde(default)]
    hq: bool,
    #[serde(default)]
    on_mannequin: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryItem {
    item_id: u32,
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum HistoryResponse {
    Multi { items: HashMap<String, HistoryItem> },
    Single(HistoryItem),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HistoryQualityMeta {
    recent_price: Option<u32>,
    daily_sales: Option<f64>,
    sales_window: Option<SalesWindow>,
}

fn normalize_item_ids(item_ids: &[u32]) -> Vec<u32> {
    let mut ids = item_ids
        .iter()
        .copied()
        .filter(|item_id| *item_id > 0)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn now_ms() -> i64 {
    js_sys::Date::now().round() as i64
}

fn cache_key(item_id: u32) -> String {
    format!("{MARKET_REGION}:{item_id}")
}

fn needs_refresh(fetched_at: i64, now: i64, ttl: i64, force: bool) -> bool {
    force || fetched_at <= 0 || now.saturating_sub(fetched_at) >= ttl
}

async fn open_market_db() -> Result<indexed_db::Database<String>, String> {
    let factory = indexed_db::Factory::get()
        .map_err(|error| format!("打开市场缓存 IndexedDB 失败: {error}"))?;
    factory
        .open(MARKET_DB_NAME, MARKET_DB_VERSION, |event| async move {
            let db = event.database();
            if !db
                .object_store_names()
                .iter()
                .any(|name| name == MARKET_STORE_NAME)
            {
                db.build_object_store(MARKET_STORE_NAME).create()?;
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("打开市场缓存数据库失败: {error}"))
}

pub async fn load_cached_market_quotes(
    item_ids: &[u32],
) -> Result<HashMap<u32, MarketQuote>, String> {
    let item_ids = normalize_item_ids(item_ids);
    if item_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let db = open_market_db().await?;
    db.transaction(&[MARKET_STORE_NAME])
        .run(move |transaction| async move {
            let store = transaction.object_store(MARKET_STORE_NAME)?;
            let mut quotes = HashMap::new();
            for item_id in item_ids {
                let key = cache_key(item_id);
                let Some(value) = store.get(&JsString::from(key.as_str())).await? else {
                    continue;
                };
                let Some(json) = value.as_string() else {
                    continue;
                };
                if let Ok(quote) = serde_json::from_str::<MarketQuote>(&json) {
                    if quote.item_id == item_id && quote.region == MARKET_REGION {
                        quotes.insert(item_id, quote);
                    }
                }
            }
            Ok(quotes)
        })
        .await
        .map_err(|error| format!("读取市场报价缓存失败: {error}"))
}

async fn save_market_quotes(quotes: &HashMap<u32, MarketQuote>) -> Result<(), String> {
    if quotes.is_empty() {
        return Ok(());
    }
    let records = quotes
        .values()
        .filter_map(|quote| {
            serde_json::to_string(quote)
                .ok()
                .map(|json| (cache_key(quote.item_id), json))
        })
        .collect::<Vec<_>>();
    let db = open_market_db().await?;
    db.transaction(&[MARKET_STORE_NAME])
        .rw()
        .run(move |transaction| async move {
            let store = transaction.object_store(MARKET_STORE_NAME)?;
            for (key, json) in records {
                store
                    .put_kv(&JsString::from(key.as_str()), &JsValue::from_str(&json))
                    .await?;
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("保存市场报价缓存失败: {error}"))
}

fn positive_price(value: Option<f64>) -> Option<u32> {
    value
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.round() as u32)
}

fn aggregated_quality(quality: &AggregatedQuality) -> (Option<u32>, Option<String>) {
    for (scope, basis) in [
        (&quality.min_listing, "最低挂单"),
        (&quality.recent_purchase, "近期成交"),
        (&quality.average_sale_price, "平均成交"),
    ] {
        let price = scope
            .as_ref()
            .and_then(|scope| scope.region.as_ref())
            .and_then(|region| positive_price(region.price));
        if price.is_some() {
            return (price, Some(basis.to_string()));
        }
    }
    (None, None)
}

fn filtered_market_price(aggregated: Option<u32>, recent: Option<u32>) -> Option<u32> {
    match (aggregated, recent) {
        (Some(aggregated), Some(recent)) if aggregated.saturating_mul(10) < recent => Some(recent),
        (Some(aggregated), _) => Some(aggregated),
        (None, recent) => recent,
    }
}

fn refresh_final_price(quality: &mut MarketQualityQuote) {
    quality.unit_price =
        filtered_market_price(quality.aggregated_unit_price, quality.recent_sale_price);
    if quality.unit_price == quality.recent_sale_price
        && quality.unit_price != quality.aggregated_unit_price
    {
        quality.basis = Some("近期成交".to_string());
    }
}

async fn fetch_aggregated(item_ids: &[u32]) -> Result<HashMap<u32, AggregatedItem>, String> {
    let mut result = HashMap::new();
    for chunk in item_ids.chunks(100) {
        let ids = chunk
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "{UNIVERSALIS_BASE_URL}/api/v2/aggregated/{}/{}",
            urlencoding::encode(MARKET_REGION),
            ids
        );
        let response = Request::get(&url)
            .send()
            .await
            .map_err(|error| format!("Universalis {error}"))?;
        if !response.ok() {
            return Err(format!("Universalis {}", response.status()));
        }
        let response = response
            .json::<AggregatedResponse>()
            .await
            .map_err(|error| format!("Universalis 报价解析失败：{error}"))?;
        for item in response.results {
            result.insert(item.item_id, item);
        }
    }
    Ok(result)
}

fn history_quality(
    entries: &[HistoryEntry],
    reference_seconds: f64,
    hq: bool,
) -> HistoryQualityMeta {
    const HALF_WINDOW_SECONDS: f64 = 3.5 * 24.0 * 60.0 * 60.0;
    let mut window = SalesWindow::default();
    let mut recent_price = None;
    let mut recent_timestamp = f64::NEG_INFINITY;
    for entry in entries {
        if entry.hq != hq
            || entry.on_mannequin
            || !entry.timestamp.is_finite()
            || !entry.quantity.is_finite()
            || !entry.price_per_unit.is_finite()
            || entry.quantity <= 0.0
            || entry.price_per_unit <= 0.0
        {
            continue;
        }
        let age = reference_seconds - entry.timestamp;
        if !(0.0..HALF_WINDOW_SECONDS * 2.0).contains(&age) {
            continue;
        }
        if age < HALF_WINDOW_SECONDS {
            window.recent += entry.quantity;
        } else {
            window.previous += entry.quantity;
        }
        if entry.timestamp > recent_timestamp {
            recent_timestamp = entry.timestamp;
            recent_price = Some(entry.price_per_unit.round() as u32);
        }
    }
    let total = window.recent + window.previous;
    HistoryQualityMeta {
        recent_price,
        daily_sales: (total > 0.0).then_some(total / 7.0),
        sales_window: (total > 0.0).then_some(window),
    }
}

async fn fetch_history(
    item_ids: &[u32],
) -> Result<HashMap<u32, (HistoryQualityMeta, HistoryQualityMeta)>, String> {
    let reference_seconds = js_sys::Date::now() / 1_000.0;
    let mut result = HashMap::new();
    for chunk in item_ids.chunks(50) {
        let ids = chunk
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!(
            "{UNIVERSALIS_BASE_URL}/api/v2/history/{}/{}?entriesWithin=604800&entriesToReturn=1000",
            urlencoding::encode(MARKET_REGION),
            ids
        );
        let response = Request::get(&url)
            .send()
            .await
            .map_err(|error| format!("Universalis 销量历史 {error}"))?;
        if !response.ok() {
            return Err(format!("Universalis 销量历史 {}", response.status()));
        }
        let body = response
            .text()
            .await
            .map_err(|error| format!("Universalis 销量历史读取失败：{error}"))?;
        let response = serde_json::from_str::<HistoryResponse>(&body)
            .map_err(|error| format!("Universalis 销量历史解析失败：{error}"))?;
        let items = match response {
            HistoryResponse::Multi { items } => items.into_values().collect(),
            HistoryResponse::Single(item) => vec![item],
        };
        for item in items {
            result.insert(
                item.item_id,
                (
                    history_quality(&item.entries, reference_seconds, false),
                    history_quality(&item.entries, reference_seconds, true),
                ),
            );
        }
    }
    Ok(result)
}

pub async fn refresh_market_quotes(item_ids: &[u32], force: bool) -> MarketRefreshResult {
    let item_ids = normalize_item_ids(item_ids);
    let mut errors = Vec::new();
    let mut quotes = match load_cached_market_quotes(&item_ids).await {
        Ok(quotes) => quotes,
        Err(error) => {
            errors.push(error);
            HashMap::new()
        }
    };
    let now = now_ms();
    let price_ids = item_ids
        .iter()
        .copied()
        .filter(|item_id| {
            quotes
                .get(item_id)
                .is_none_or(|quote| needs_refresh(quote.price_fetched_at, now, PRICE_TTL_MS, force))
        })
        .collect::<Vec<_>>();
    let history_ids = item_ids
        .iter()
        .copied()
        .filter(|item_id| {
            quotes.get(item_id).is_none_or(|quote| {
                needs_refresh(quote.history_fetched_at, now, HISTORY_TTL_MS, force)
            })
        })
        .collect::<Vec<_>>();

    if !price_ids.is_empty() {
        match fetch_aggregated(&price_ids).await {
            Ok(fetched) => {
                for item_id in &price_ids {
                    let quote = quotes.entry(*item_id).or_insert_with(|| MarketQuote {
                        region: MARKET_REGION.to_string(),
                        item_id: *item_id,
                        ..Default::default()
                    });
                    quote.price_fetched_at = now;
                    if let Some(item) = fetched.get(item_id) {
                        let (nq_price, nq_basis) = aggregated_quality(&item.nq);
                        let (hq_price, hq_basis) = aggregated_quality(&item.hq);
                        quote.nq.aggregated_unit_price = nq_price;
                        quote.nq.basis = nq_basis;
                        quote.hq.aggregated_unit_price = hq_price;
                        quote.hq.basis = hq_basis;
                        quote.updated_at = item
                            .world_upload_times
                            .iter()
                            .map(|upload| upload.timestamp)
                            .filter(|timestamp| timestamp.is_finite() && *timestamp > 0.0)
                            .max_by(f64::total_cmp)
                            .map(|timestamp| timestamp.round() as i64);
                        refresh_final_price(&mut quote.nq);
                        refresh_final_price(&mut quote.hq);
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }

    if !history_ids.is_empty() {
        match fetch_history(&history_ids).await {
            Ok(fetched) => {
                for item_id in &history_ids {
                    let quote = quotes.entry(*item_id).or_insert_with(|| MarketQuote {
                        region: MARKET_REGION.to_string(),
                        item_id: *item_id,
                        ..Default::default()
                    });
                    quote.history_fetched_at = now;
                    if let Some((nq, hq)) = fetched.get(item_id) {
                        quote.nq.recent_sale_price = nq.recent_price;
                        quote.nq.daily_sales = nq.daily_sales;
                        quote.nq.sales_window = nq.sales_window;
                        quote.hq.recent_sale_price = hq.recent_price;
                        quote.hq.daily_sales = hq.daily_sales;
                        quote.hq.sales_window = hq.sales_window;
                        refresh_final_price(&mut quote.nq);
                        refresh_final_price(&mut quote.hq);
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }

    if let Err(error) = save_market_quotes(&quotes).await {
        errors.push(error);
    }
    MarketRefreshResult {
        quotes,
        error: (!errors.is_empty()).then(|| errors.join("；")),
    }
}

fn sum_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn combine_windows(left: Option<SalesWindow>, right: Option<SalesWindow>) -> Option<SalesWindow> {
    match (left, right) {
        (Some(left), Some(right)) => Some(SalesWindow {
            recent: left.recent + right.recent,
            previous: left.previous + right.previous,
        }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mannequin_sales_are_excluded_from_shared_history() {
        let entries = vec![
            HistoryEntry {
                timestamp: 999_900.0,
                quantity: 99.0,
                price_per_unit: 1.0,
                hq: true,
                on_mannequin: true,
            },
            HistoryEntry {
                timestamp: 999_800.0,
                quantity: 2.0,
                price_per_unit: 200_000.0,
                hq: true,
                on_mannequin: false,
            },
        ];
        let meta = history_quality(&entries, 1_000_000.0, true);
        assert_eq!(meta.recent_price, Some(200_000));
        assert_eq!(meta.daily_sales, Some(2.0 / 7.0));
    }

    #[test]
    fn suspicious_aggregated_price_uses_recent_non_mannequin_sale() {
        assert_eq!(filtered_market_price(Some(1), Some(200_000)), Some(200_000));
        assert_eq!(
            filtered_market_price(Some(160_000), Some(34_444)),
            Some(160_000)
        );
    }

    #[test]
    fn cache_ttl_and_force_refresh_are_independent() {
        assert!(!needs_refresh(900, 1_000, 200, false));
        assert!(needs_refresh(700, 1_000, 200, false));
        assert!(needs_refresh(900, 1_000, 200, true));
        assert!(needs_refresh(0, 1_000, 200, false));
    }

    #[test]
    fn persisted_quote_round_trips_all_shared_fields() {
        let quote = MarketQuote {
            region: MARKET_REGION.to_string(),
            item_id: 42,
            updated_at: Some(1_000),
            nq: MarketQualityQuote {
                unit_price: Some(120),
                basis: Some("最低挂单".to_string()),
                daily_sales: Some(2.5),
                sales_window: Some(SalesWindow {
                    recent: 10.0,
                    previous: 7.0,
                }),
                aggregated_unit_price: Some(120),
                recent_sale_price: Some(125),
            },
            price_fetched_at: 2_000,
            history_fetched_at: 3_000,
            ..Default::default()
        };
        let json = serde_json::to_string(&quote).unwrap();
        assert_eq!(serde_json::from_str::<MarketQuote>(&json).unwrap(), quote);
    }
}
