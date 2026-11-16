use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): [mean_mu, variance_sigma2, last_price]
static OU_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 1.0, 0.0); 3]);

// ENGINE 18: Tier-0 Ornstein-Uhlenbeck (O-U) Stochastic Mean Reversion Scalper
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("v2/ticker") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let idx = if symbol.contains("BTC") {
        0
    } else if symbol.contains("ETH") {
        1
    } else if symbol.contains("SOL") {
        2
    } else {
        return;
    };

    let price = parsed
        .get("mark_price")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = OU_STATE.lock() {
        let (prev_mu, prev_var, last_p) = state[idx];

        if prev_mu <= 0.0 {
            state[idx] = (price, 1.0, price);
            return;
        }

        // O-U Parameters via nalgebra Vector3: [Equilibrium Mean (mu), Variance (sigma^2), Pull Speed (theta)]
        let theta = 0.18; // Mean reversion spring constant
        let new_mu = prev_mu + theta * (price - prev_mu);
        let diff = price - new_mu;
        let new_var = ((prev_var * 0.85) + (diff * diff * 0.15)).max(1e-6);

        let ou_vec = Vector3::new(new_mu, new_var.sqrt(), theta * (new_mu - price));
        let z_score = (price - ou_vec[0]) / ou_vec[1];
        let pull_bps = (ou_vec[2] / price) * 10000.0;

        let ou_signal = if z_score > 2.0 {
            "🪃 [OU_OVERBOUGHT_SNAPBACK_SHORT]"
        } else if z_score < -2.0 {
            "🪃 [OU_OVERSOLD_SNAPBACK_LONG]"
        } else {
            "⚖️ [OU_EQUILIBRIUM_ZONE]"
        };

        println!(
            "🧲 [E18:{}] OU_MEAN:{:.2} | Z_SCORE:{:+.2} | PULL:{:+.2}bps -> {}",
            symbol, ou_vec[0], z_score, pull_bps, ou_signal
        );

        state[idx] = (new_mu, new_var, price);
    }
}
