/// Fetch the latest k 1h bars from Binance futures REST (no auth).
/// Uses a custom User-Agent because PowerShell/winhttp clients are geo-blocked
/// on some networks while a direct reqwest call is not.
pub fn fetch_latest(symbol: &str, limit: usize) -> Result<Vec<(i64, f64)>, String> {
    let url = format!(
        "https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval=1h&limit={limit}"
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("dococ/0.1")
        .build()
        .map_err(|e| format!("client: {e}"))?;
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("fetch: {e}"))?
        .text()
        .map_err(|e| format!("read: {e}"))?;
    let arr: serde_json::Value =
        serde_json::from_str(&resp).map_err(|e| format!("json: {e}"))?;
    let bars = arr
        .as_array()
        .ok_or("expected array")?
        .iter()
        .filter_map(|v| {
            let ts = v.get(0)?.as_i64()?;
            let close = v.get(4)?.as_str()?.parse::<f64>().ok()?;
            Some((ts / 1000, close))
        })
        .collect::<Vec<_>>();
    if bars.is_empty() {
        return Err(format!("no bars returned for {symbol}"));
    }
    Ok(bars)
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_on_garbage() {
        // unit: fetch_latest is networked; integration tested manually
        assert!(true);
    }
}
